//! Lettura e scrittura delle impostazioni nella tabella `impostazioni` (chiave -> JSON).

use rusqlite::params;
use serde_json::{Map, Value};

use super::Archivio;
use crate::impostazioni::Impostazioni;

impl Archivio {
    /// Legge le impostazioni; le chiavi mancanti prendono il valore predefinito.
    pub fn leggi_impostazioni(&self) -> Result<Impostazioni, String> {
        let conn = self.conn();
        let mut st = conn
            .prepare("SELECT chiave, valore FROM impostazioni")
            .map_err(|e| e.to_string())?;
        let righe = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?;
        let mut mappa = Map::new();
        for riga in righe {
            let (chiave, valore) = riga.map_err(|e| e.to_string())?;
            if let Ok(v) = serde_json::from_str::<Value>(&valore) {
                mappa.insert(chiave, v);
            }
        }
        // Un valore corrotto non deve impedire l'avvio: si torna ai predefiniti
        Ok(serde_json::from_value::<Impostazioni>(Value::Object(mappa))
            .unwrap_or_default()
            .validata())
    }

    /// Salva tutte le impostazioni (una riga per campo, in una transazione).
    pub fn salva_impostazioni(&self, imp: &Impostazioni) -> Result<(), String> {
        let Value::Object(mappa) = serde_json::to_value(imp).map_err(|e| e.to_string())? else {
            return Err("Impostazioni non serializzabili".into());
        };
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        for (chiave, valore) in mappa {
            tx.execute(
                "INSERT INTO impostazioni (chiave, valore) VALUES (?1, ?2)
                 ON CONFLICT(chiave) DO UPDATE SET valore = excluded.valore",
                params![chiave, valore.to_string()],
            )
            .map_err(|e| e.to_string())?;
        }
        tx.commit().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::impostazioni::ModalitaConsumo;
    use crate::testi::Lingua;

    #[test]
    fn database_vuoto_da_i_predefiniti() {
        let a = Archivio::in_memoria().unwrap();
        assert_eq!(a.leggi_impostazioni().unwrap(), Impostazioni::default());
    }

    #[test]
    fn salva_e_rilegge() {
        let a = Archivio::in_memoria().unwrap();
        let imp = Impostazioni {
            monitoraggio_attivo: false,
            intervallo_sicurezza_min: 5,
            speedtest_ogni_ore: Some(6),
            server_speedtest: Some(22857),
            ripetizioni_speedtest: 3,
            soglia_download_min_mbps: Some(40.0),
            soglia_ping_max_ms: None,
            chiudi_in_tray: false,
            avvio_con_windows: true,
            notifiche_attive: false,
            modalita_consumo: ModalitaConsumo::Ignora,
            licenza_ookla_accettata: true,
            lingua: Lingua::En,
            controlla_aggiornamenti: false,
        };
        a.salva_impostazioni(&imp).unwrap();
        assert_eq!(a.leggi_impostazioni().unwrap(), imp);
        // Seconda scrittura: aggiorna, non duplica
        let imp2 = Impostazioni { intervallo_sicurezza_min: 60, ..imp.clone() };
        a.salva_impostazioni(&imp2).unwrap();
        assert_eq!(a.leggi_impostazioni().unwrap(), imp2);
        let n: i64 = a
            .conn()
            .query_row("SELECT COUNT(*) FROM impostazioni", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 14);
    }

    #[test]
    fn valore_corrotto_torna_ai_predefiniti() {
        let a = Archivio::in_memoria().unwrap();
        a.conn()
            .execute("INSERT INTO impostazioni (chiave, valore) VALUES ('intervalloSicurezzaMin', '\"abc\"')", [])
            .unwrap();
        assert_eq!(a.leggi_impostazioni().unwrap(), Impostazioni::default());
    }
}
