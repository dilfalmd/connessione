//! Migrazioni dello schema, tracciate con `PRAGMA user_version`.

use rusqlite::Connection;

/// Ogni elemento porta lo schema alla versione `indice + 1`. Si aggiungono in coda, mai si modificano.
const MIGRAZIONI: &[&str] = &[
    // 1: schema iniziale
    "CREATE TABLE misure_velocita (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp      TEXT NOT NULL,
        sessione       TEXT NOT NULL,
        isp            TEXT,
        ip_esterno     TEXT,
        server_id      INTEGER,
        server_nome    TEXT,
        server_luogo   TEXT,
        ping_ms        REAL NOT NULL,
        jitter_ms      REAL,
        download_mbps  REAL NOT NULL,
        upload_mbps    REAL NOT NULL,
        packet_loss    REAL,
        url            TEXT,
        vpn_attiva     INTEGER,
        origine        TEXT NOT NULL CHECK (origine IN ('app', 'import'))
    );
    CREATE INDEX idx_misure_velocita_timestamp ON misure_velocita (timestamp);
    CREATE INDEX idx_misure_velocita_sessione ON misure_velocita (sessione);
    CREATE TABLE controlli_sicurezza (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        timestamp    TEXT NOT NULL,
        verdetto     TEXT NOT NULL,
        punteggio    INTEGER NOT NULL,
        ipv4         TEXT,
        vpn_attiva   INTEGER,
        report_json  TEXT NOT NULL
    );
    CREATE INDEX idx_controlli_sicurezza_timestamp ON controlli_sicurezza (timestamp);",
    // 2: misure collegate al controllo di sicurezza del check completo
    "ALTER TABLE misure_velocita ADD COLUMN controllo_id INTEGER REFERENCES controlli_sicurezza (id);
    CREATE INDEX idx_misure_velocita_controllo ON misure_velocita (controllo_id);",
    // 3: impostazioni chiave/valore (valore = JSON)
    "CREATE TABLE impostazioni (
        chiave  TEXT PRIMARY KEY,
        valore  TEXT NOT NULL
    );",
];

#[cfg(test)]
pub const VERSIONE_CORRENTE: i64 = MIGRAZIONI.len() as i64;

/// Applica le migrazioni mancanti, ciascuna in una transazione.
pub fn applica(conn: &mut Connection) -> rusqlite::Result<()> {
    let versione: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (indice, sql) in MIGRAZIONI.iter().enumerate() {
        let target = indice as i64 + 1;
        if versione >= target {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", target)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabelle(conn: &Connection) -> Vec<String> {
        let mut st = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .unwrap();
        st.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    }

    #[test]
    fn database_nuovo_arriva_alla_versione_corrente() {
        let mut conn = Connection::open_in_memory().unwrap();
        applica(&mut conn).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, VERSIONE_CORRENTE);
        assert_eq!(tabelle(&conn), vec!["controlli_sicurezza", "impostazioni", "misure_velocita"]);
    }

    #[test]
    fn rieseguire_le_migrazioni_non_cambia_nulla() {
        let mut conn = Connection::open_in_memory().unwrap();
        applica(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO controlli_sicurezza (timestamp, verdetto, punteggio, report_json) VALUES ('t','PROTECTED',100,'{}')",
            [],
        )
        .unwrap();
        applica(&mut conn).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM controlli_sicurezza", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, VERSIONE_CORRENTE);
    }

    #[test]
    fn origine_non_valida_rifiutata() {
        let mut conn = Connection::open_in_memory().unwrap();
        applica(&mut conn).unwrap();
        let r = conn.execute(
            "INSERT INTO misure_velocita (timestamp, sessione, ping_ms, download_mbps, upload_mbps, origine) VALUES ('t','s',1,1,1,'altro')",
            [],
        );
        assert!(r.is_err());
    }

    #[test]
    fn colonne_della_tabella_velocita() {
        let mut conn = Connection::open_in_memory().unwrap();
        applica(&mut conn).unwrap();
        let mut st = conn.prepare("PRAGMA table_info(misure_velocita)").unwrap();
        let nomi: Vec<String> = st
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(
            nomi,
            vec![
                "id", "timestamp", "sessione", "isp", "ip_esterno", "server_id", "server_nome",
                "server_luogo", "ping_ms", "jitter_ms", "download_mbps", "upload_mbps",
                "packet_loss", "url", "vpn_attiva", "origine", "controllo_id"
            ]
        );
    }

    #[test]
    fn migrazione_2_parte_da_un_database_alla_versione_1() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Simula un database creato dal lotto 2: solo la migrazione 1
        let tx = conn.transaction().unwrap();
        tx.execute_batch(MIGRAZIONI[0]).unwrap();
        tx.pragma_update(None, "user_version", 1).unwrap();
        tx.commit().unwrap();
        conn.execute(
            "INSERT INTO misure_velocita (timestamp, sessione, ping_ms, download_mbps, upload_mbps, origine) VALUES ('t','s',1,2,3,'app')",
            [],
        )
        .unwrap();

        applica(&mut conn).unwrap();

        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, VERSIONE_CORRENTE);
        // La riga esistente resta, con controllo_id NULL
        let c: Option<i64> = conn
            .query_row("SELECT controllo_id FROM misure_velocita", [], |r| r.get(0))
            .unwrap();
        assert_eq!(c, None);
        // La chiave esterna e` rispettata solo se la si attiva; il collegamento funziona
        conn.execute(
            "INSERT INTO controlli_sicurezza (timestamp, verdetto, punteggio, report_json) VALUES ('t','PROTECTED',100,'{}')",
            [],
        )
        .unwrap();
        conn.execute("UPDATE misure_velocita SET controllo_id = 1", []).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM misure_velocita WHERE controllo_id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn migrazione_3_aggiunge_le_impostazioni_senza_toccare_i_dati() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Database creato dal lotto 3: migrazioni 1 e 2
        for (i, sql) in MIGRAZIONI.iter().take(2).enumerate() {
            let tx = conn.transaction().unwrap();
            tx.execute_batch(sql).unwrap();
            tx.pragma_update(None, "user_version", i as i64 + 1).unwrap();
            tx.commit().unwrap();
        }
        conn.execute(
            "INSERT INTO controlli_sicurezza (timestamp, verdetto, punteggio, report_json) VALUES ('t','PROTECTED',100,'{}')",
            [],
        )
        .unwrap();
        applica(&mut conn).unwrap();
        let v: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 3);
        assert_eq!(VERSIONE_CORRENTE, 3);
        conn.execute("INSERT INTO impostazioni (chiave, valore) VALUES ('k', 'true')", []).unwrap();
        let dup = conn.execute("INSERT INTO impostazioni (chiave, valore) VALUES ('k', 'false')", []);
        assert!(dup.is_err(), "la chiave e` univoca");
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM controlli_sicurezza", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }
}
