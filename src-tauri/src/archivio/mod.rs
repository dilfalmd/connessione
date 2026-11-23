//! Archivio SQLite (`connessione.db`): misure di velocità e controlli di sicurezza.

mod esporta;
mod impostazioni_db;
mod import_csv;
mod migrazioni;
mod sicurezza;
mod statistiche;
mod velocita;

use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;

pub use sicurezza::ControlloSicurezza;
pub use statistiche::GruppoVpn;
pub use velocita::MisuraVelocita;

pub struct Archivio {
    conn: Mutex<Connection>,
}

impl Archivio {
    /// Apre (o crea) il database su file e applica le migrazioni.
    pub fn apri(percorso: &Path) -> Result<Self, String> {
        let conn = Connection::open(percorso).map_err(|e| e.to_string())?;
        Self::da_connessione(conn)
    }

    /// Database in memoria (test).
    #[cfg(test)]
    pub fn in_memoria() -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        Self::da_connessione(conn)
    }

    fn da_connessione(mut conn: Connection) -> Result<Self, String> {
        migrazioni::applica(&mut conn).map_err(|e| e.to_string())?;
        Ok(Archivio {
            conn: Mutex::new(conn),
        })
    }

    pub(crate) fn conn(&self) -> MutexGuard<'_, Connection> {
        // Un panic in un'altra operazione non deve rendere l'archivio inutilizzabile.
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Versione dello schema (PRAGMA user_version).
    #[cfg(test)]
    pub fn versione_schema(&self) -> Result<i64, String> {
        self.conn()
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archivio_su_file_conserva_i_dati_e_la_versione() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("connessione.db");
        {
            let a = Archivio::apri(&file).unwrap();
            assert_eq!(a.versione_schema().unwrap(), migrazioni::VERSIONE_CORRENTE);
            a.inserisci_misura(&MisuraVelocita {
                id: None,
                timestamp: "2026-11-23T10:00:00+01:00".into(),
                sessione: "s".into(),
                isp: None,
                ip_esterno: None,
                server_id: None,
                server_nome: None,
                server_luogo: None,
                ping_ms: 1.0,
                jitter_ms: None,
                download_mbps: 2.0,
                upload_mbps: 3.0,
                packet_loss: None,
                url: None,
                vpn_attiva: None,
                controllo_id: None,
                origine: "app".into(),
            })
            .unwrap();
        }
        let a = Archivio::apri(&file).unwrap();
        assert_eq!(a.conta_misure().unwrap(), 1);
        assert_eq!(a.versione_schema().unwrap(), migrazioni::VERSIONE_CORRENTE);
    }
}
