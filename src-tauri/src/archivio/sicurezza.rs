//! Tabella `controlli_sicurezza`.

use rusqlite::{Row, params};
use serde::Serialize;
use serde_json::Value;

use super::Archivio;
use crate::scanner::SecurityReport;
use crate::scanner::report::OverallStatus;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlloSicurezza {
    pub id: i64,
    pub timestamp: String,
    pub verdetto: String,
    pub punteggio: i64,
    pub ipv4: Option<String>,
    pub vpn_attiva: Option<bool>,
    /// Report completo com'era al momento del controllo (JSON grezzo: lo schema può evolvere).
    pub report: Value,
}

fn da_riga(r: &Row<'_>) -> rusqlite::Result<ControlloSicurezza> {
    let json: String = r.get("report_json")?;
    Ok(ControlloSicurezza {
        id: r.get("id")?,
        timestamp: r.get("timestamp")?,
        verdetto: r.get("verdetto")?,
        punteggio: r.get("punteggio")?,
        ipv4: r.get("ipv4")?,
        vpn_attiva: r.get("vpn_attiva")?,
        report: serde_json::from_str(&json).unwrap_or(Value::Null),
    })
}

/// Vero se il report va nello storico (cioè l'esito complessivo non è «Errore»).
pub(super) fn da_salvare(report: &SecurityReport) -> bool {
    report.overall_status != OverallStatus::Error
}

impl Archivio {
    /// Salva un report; restituisce l'id della riga.
    pub fn salva_controllo(&self, report: &SecurityReport) -> Result<i64, String> {
        let json = serde_json::to_string(report).map_err(|e| e.to_string())?;
        let verdetto = serde_json::to_value(report.overall_status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        let conn = self.conn();
        conn.execute(
            "INSERT INTO controlli_sicurezza (timestamp, verdetto, punteggio, ipv4, vpn_attiva, report_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                report.timestamp,
                verdetto,
                report.security_score,
                report.ipv4,
                // Stato VPN ignoto -> NULL: la dashboard mostra "sconosciuto"
                (!report.vpn_sconosciuta).then_some(report.is_vpn_or_proxy_ip),
                json
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    }

    /// Salva il report solo se il controllo è andato a buon fine: un esito complessivo «Errore»
    /// (servizi non raggiungibili, nessun dato affidabile) non entra nello storico.
    /// `None` se non salvato (esito «Errore» o errore del database, che viene registrato).
    pub fn salva_se_valido(&self, report: &SecurityReport) -> Option<i64> {
        if !da_salvare(report) {
            return None;
        }
        match self.salva_controllo(report) {
            Ok(id) => Some(id),
            Err(e) => {
                eprintln!("Salvataggio del controllo di sicurezza non riuscito: {e}");
                None
            }
        }
    }

    pub fn storico_sicurezza(&self, da: Option<&str>, a: Option<&str>) -> Result<Vec<ControlloSicurezza>, String> {
        let conn = self.conn();
        let mut st = conn
            .prepare(
                "SELECT * FROM controlli_sicurezza
                 WHERE (?1 IS NULL OR datetime(timestamp) >= datetime(?1))
                   AND (?2 IS NULL OR datetime(timestamp) <= datetime(?2))
                 ORDER BY datetime(timestamp), id",
            )
            .map_err(|e| e.to_string())?;
        let righe = st
            .query_map(params![da, a], da_riga)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(righe)
    }

    /// Gli ultimi `n` controlli, dal più recente.
    pub fn ultimi_controlli(&self, n: u32) -> Result<Vec<ControlloSicurezza>, String> {
        let conn = self.conn();
        let mut st = conn
            .prepare("SELECT * FROM controlli_sicurezza ORDER BY datetime(timestamp) DESC, id DESC LIMIT ?1")
            .map_err(|e| e.to_string())?;
        let righe = st
            .query_map(params![n], da_riga)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(righe)
    }

    /// Il controllo con l'id dato, oppure il più recente se `id` è `None`.
    pub fn controllo(&self, id: Option<i64>) -> Result<Option<ControlloSicurezza>, String> {
        let conn = self.conn();
        let mut st = conn
            .prepare(
                "SELECT * FROM controlli_sicurezza
                 WHERE (?1 IS NULL OR id = ?1)
                 ORDER BY datetime(timestamp) DESC, id DESC LIMIT 1",
            )
            .map_err(|e| e.to_string())?;
        let mut righe = st.query_map(params![id], da_riga).map_err(|e| e.to_string())?;
        righe.next().transpose().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i_controlli_in_errore_non_si_salvano() {
        let a = Archivio::in_memoria().unwrap();
        let mut errore = SecurityReport::vuoto("2026-11-23T10:00:00+01:00".into());
        errore.overall_status = OverallStatus::Error;
        assert!(!da_salvare(&errore));
        assert_eq!(a.salva_se_valido(&errore), None);
        assert!(a.storico_sicurezza(None, None).unwrap().is_empty());

        for stato in [OverallStatus::Protected, OverallStatus::Warning, OverallStatus::Unprotected] {
            let mut r = SecurityReport::vuoto("2026-11-23T10:05:00+01:00".into());
            r.overall_status = stato;
            assert!(da_salvare(&r));
            assert!(a.salva_se_valido(&r).is_some());
        }
        assert_eq!(a.storico_sicurezza(None, None).unwrap().len(), 3);
    }

    #[test]
    fn salva_e_rilegge_un_report() {
        let a = Archivio::in_memoria().unwrap();
        let mut r = SecurityReport::vuoto("2026-11-23T10:00:00+01:00".into());
        r.ipv4 = Some("203.0.113.7".into());
        r.security_score = 35;
        r.is_vpn_or_proxy_ip = false;
        let id = a.salva_controllo(&r).unwrap();
        let v = a.storico_sicurezza(None, None).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, id);
        assert_eq!(v[0].verdetto, "UNPROTECTED");
        assert_eq!(v[0].punteggio, 35);
        assert_eq!(v[0].vpn_attiva, Some(false));
        assert_eq!(v[0].report["ipv4"], "203.0.113.7");
        assert_eq!(v[0].report["securityScore"], 35);
        assert!(
            a.storico_sicurezza(Some("2026-12-01T00:00:00+01:00"), None)
                .unwrap()
                .is_empty()
        );
    }
}
