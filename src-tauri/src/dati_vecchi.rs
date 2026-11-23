//! Migrazione dei dati dal vecchio identificatore (`local.connessione`) a quello attuale
//! (`it.dilettafalcone.connessione`): la cartella dati dell'app cambia nome con l'identificatore.

use std::fs;
use std::path::Path;

/// Nome della cartella dati con il vecchio identificatore (sorella di quella attuale).
pub const CARTELLA_VECCHIA: &str = "local.connessione";
const FILE_DB: &str = "connessione.db";
/// File accessori di SQLite, se esistono vanno copiati insieme al database.
const ACCESSORI: [&str; 2] = ["connessione.db-wal", "connessione.db-shm"];

/// Se `nuova` non contiene `connessione.db` e `vecchia` sì, copia (senza spostare) il database
/// (che contiene anche le impostazioni) nella cartella nuova. `Ok(true)` se ha copiato.
pub fn copia_dati_vecchi(nuova: &Path, vecchia: &Path) -> Result<bool, String> {
    let db_nuovo = nuova.join(FILE_DB);
    let db_vecchio = vecchia.join(FILE_DB);
    if db_nuovo.exists() || !db_vecchio.is_file() {
        return Ok(false);
    }
    fs::create_dir_all(nuova).map_err(|e| e.to_string())?;
    // Prima gli accessori, il database per ultimo (e con un nome provvisorio): se la copia si
    // interrompe, `connessione.db` non esiste a metà e al prossimo avvio si ripete.
    for nome in ACCESSORI {
        let da = vecchia.join(nome);
        if da.is_file() {
            fs::copy(&da, nuova.join(nome)).map_err(|e| format!("{nome}: {e}"))?;
        }
    }
    let provvisorio = nuova.join(format!("{FILE_DB}.copia"));
    fs::copy(&db_vecchio, &provvisorio).map_err(|e| e.to_string())?;
    fs::rename(&provvisorio, &db_nuovo).map_err(|e| e.to_string())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copia_il_database_senza_spostarlo() {
        let base = tempfile::tempdir().unwrap();
        let vecchia = base.path().join(CARTELLA_VECCHIA);
        let nuova = base.path().join("it.dilettafalcone.connessione");
        fs::create_dir_all(&vecchia).unwrap();
        fs::write(vecchia.join(FILE_DB), b"dati vecchi").unwrap();

        assert_eq!(copia_dati_vecchi(&nuova, &vecchia), Ok(true));
        assert_eq!(fs::read(nuova.join(FILE_DB)).unwrap(), b"dati vecchi");
        assert!(vecchia.join(FILE_DB).is_file(), "l'originale resta dov'è");
        assert!(!nuova.join(format!("{FILE_DB}.copia")).exists());
    }

    #[test]
    fn non_sovrascrive_un_database_nuovo_gia_presente() {
        let base = tempfile::tempdir().unwrap();
        let vecchia = base.path().join(CARTELLA_VECCHIA);
        let nuova = base.path().join("nuova");
        fs::create_dir_all(&vecchia).unwrap();
        fs::create_dir_all(&nuova).unwrap();
        fs::write(vecchia.join(FILE_DB), b"vecchio").unwrap();
        fs::write(nuova.join(FILE_DB), b"nuovo").unwrap();
        assert_eq!(copia_dati_vecchi(&nuova, &vecchia), Ok(false));
        assert_eq!(fs::read(nuova.join(FILE_DB)).unwrap(), b"nuovo");
    }

    #[test]
    fn senza_vecchia_cartella_non_fa_nulla() {
        let base = tempfile::tempdir().unwrap();
        let nuova = base.path().join("nuova");
        assert_eq!(copia_dati_vecchi(&nuova, &base.path().join("manca")), Ok(false));
        assert!(!nuova.exists());
    }
}
