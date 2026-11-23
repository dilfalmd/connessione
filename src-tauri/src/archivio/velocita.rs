//! Tabella `misure_velocita`.

use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};

use super::Archivio;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MisuraVelocita {
    /// `None` finché non è stata inserita nel database.
    pub id: Option<i64>,
    /// ISO 8601 con fuso orario locale.
    pub timestamp: String,
    /// Uguale per tutte le ripetizioni di uno stesso avvio.
    pub sessione: String,
    pub isp: Option<String>,
    pub ip_esterno: Option<String>,
    pub server_id: Option<i64>,
    pub server_nome: Option<String>,
    pub server_luogo: Option<String>,
    pub ping_ms: f64,
    pub jitter_ms: Option<f64>,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub packet_loss: Option<f64>,
    pub url: Option<String>,
    pub vpn_attiva: Option<bool>,
    /// Controllo di sicurezza collegato (check completo); `None` per le misure isolate.
    #[serde(default)]
    pub controllo_id: Option<i64>,
    /// "app" oppure "import"
    pub origine: String,
}

fn da_riga(r: &Row<'_>) -> rusqlite::Result<MisuraVelocita> {
    Ok(MisuraVelocita {
        id: Some(r.get("id")?),
        timestamp: r.get("timestamp")?,
        sessione: r.get("sessione")?,
        isp: r.get("isp")?,
        ip_esterno: r.get("ip_esterno")?,
        server_id: r.get("server_id")?,
        server_nome: r.get("server_nome")?,
        server_luogo: r.get("server_luogo")?,
        ping_ms: r.get("ping_ms")?,
        jitter_ms: r.get("jitter_ms")?,
        download_mbps: r.get("download_mbps")?,
        upload_mbps: r.get("upload_mbps")?,
        packet_loss: r.get("packet_loss")?,
        url: r.get("url")?,
        vpn_attiva: r.get("vpn_attiva")?,
        controllo_id: r.get("controllo_id")?,
        origine: r.get("origine")?,
    })
}

/// Inserisce una misura su una connessione (o transazione) già aperta.
pub(super) fn inserisci_su(conn: &Connection, m: &MisuraVelocita) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO misure_velocita (timestamp, sessione, isp, ip_esterno, server_id, server_nome,
            server_luogo, ping_ms, jitter_ms, download_mbps, upload_mbps, packet_loss, url,
            vpn_attiva, origine, controllo_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
        params![
            m.timestamp, m.sessione, m.isp, m.ip_esterno, m.server_id, m.server_nome,
            m.server_luogo, m.ping_ms, m.jitter_ms, m.download_mbps, m.upload_mbps,
            m.packet_loss, m.url, m.vpn_attiva, m.origine, m.controllo_id
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Esiste già una misura con lo stesso timestamp e lo stesso url (NULL = NULL)?
pub(super) fn esiste_su(conn: &Connection, timestamp: &str, url: Option<&str>) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM misure_velocita WHERE timestamp = ?1 AND url IS ?2)",
        params![timestamp, url],
        |r| r.get(0),
    )
    .map_err(|e| e.to_string())
}

impl Archivio {
    /// Inserisce una misura e restituisce l'id assegnato.
    pub fn inserisci_misura(&self, m: &MisuraVelocita) -> Result<i64, String> {
        inserisci_su(&self.conn(), m)
    }

    /// Misure comprese fra `da` e `a` (ISO 8601, estremi inclusi, entrambi facoltativi),
    /// in ordine cronologico. Il confronto normalizza i fusi orari con `datetime()` di SQLite.
    pub fn storico_velocita(&self, da: Option<&str>, a: Option<&str>) -> Result<Vec<MisuraVelocita>, String> {
        let conn = self.conn();
        let mut st = conn
            .prepare(
                "SELECT * FROM misure_velocita
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

    #[cfg(test)]
    pub fn conta_misure(&self) -> Result<i64, String> {
        self.conn()
            .query_row("SELECT COUNT(*) FROM misure_velocita", [], |r| r.get(0))
            .map_err(|e| e.to_string())
    }


    /// Misure collegate a un controllo (check completo), in ordine cronologico.
    pub fn misure_per_controllo(&self, controllo_id: i64) -> Result<Vec<MisuraVelocita>, String> {
        let conn = self.conn();
        let mut st = conn
            .prepare("SELECT * FROM misure_velocita WHERE controllo_id = ?1 ORDER BY datetime(timestamp), id")
            .map_err(|e| e.to_string())?;
        let righe = st
            .query_map(params![controllo_id], da_riga)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        Ok(righe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn esempio(timestamp: &str, download: f64) -> MisuraVelocita {
        MisuraVelocita {
            id: None,
            timestamp: timestamp.into(),
            sessione: "sess-1".into(),
            isp: Some("Fastweb".into()),
            ip_esterno: Some("203.0.113.7".into()),
            server_id: Some(22857),
            server_nome: Some("Server Esempio".into()),
            server_luogo: Some("Milano".into()),
            ping_ms: 20.5,
            jitter_ms: Some(1.2),
            download_mbps: download,
            upload_mbps: 10.0,
            packet_loss: None,
            url: Some(format!("https://example.test/{timestamp}")),
            vpn_attiva: Some(false),
            controllo_id: None,
            origine: "app".into(),
        }
    }

    #[test]
    fn inserimento_e_lettura() {
        let a = Archivio::in_memoria().unwrap();
        let id = a.inserisci_misura(&esempio("2026-11-23T10:00:00+01:00", 50.0)).unwrap();
        let v = a.storico_velocita(None, None).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].id, Some(id));
        assert_eq!(v[0].packet_loss, None);
        assert_eq!(v[0].vpn_attiva, Some(false));
        assert_eq!(a.conta_misure().unwrap(), 1);
    }

    #[test]
    fn filtro_per_intervallo_con_fusi_orari_diversi() {
        let a = Archivio::in_memoria().unwrap();
        // Tutte alle 09:00 UTC, scritte con offset diversi
        a.inserisci_misura(&esempio("2026-11-23T10:00:00+01:00", 1.0)).unwrap();
        a.inserisci_misura(&esempio("2026-07-01T11:00:00+02:00", 2.0)).unwrap();
        a.inserisci_misura(&esempio("2026-01-15T12:00:00+01:00", 3.0)).unwrap();
        let tutte = a.storico_velocita(None, None).unwrap();
        assert_eq!(tutte.iter().map(|m| m.download_mbps).collect::<Vec<_>>(), vec![3.0, 2.0, 1.0]);
        let da = a.storico_velocita(Some("2026-06-01T00:00:00+02:00"), None).unwrap();
        assert_eq!(da.len(), 2);
        let fra = a
            .storico_velocita(Some("2026-06-01T00:00:00+02:00"), Some("2026-08-01T00:00:00+02:00"))
            .unwrap();
        assert_eq!(fra.len(), 1);
        assert_eq!(fra[0].download_mbps, 2.0);
    }

    #[test]
    fn serializzazione_camel_case() {
        let j = serde_json::to_value(esempio("2026-11-23T10:00:00+01:00", 5.0)).unwrap();
        assert_eq!(j["downloadMbps"], 5.0);
        assert_eq!(j["ipEsterno"], "203.0.113.7");
        assert!(j["packetLoss"].is_null());
    }
}
