//! Localizzazione e uso dell'eseguibile Ookla Speedtest CLI.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use super::parsing::parse_elenco_server;
use super::tipi::{ErroreSpeedtest, ServerSpeedtest};
use crate::testi::tr;

const NOME_EXE: &str = "speedtest.exe";
const PREFISSO_PACCHETTO_WINGET: &str = "Ookla.Speedtest.CLI_";
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Cerca `speedtest.exe` nelle cartelle di `path_var`, poi nei pacchetti WinGet
/// (`<local_app_data>\Microsoft\WinGet\Packages\Ookla.Speedtest.CLI_*\speedtest.exe`).
pub fn trova_in(path_var: Option<&OsString>, local_app_data: Option<&Path>) -> Option<PathBuf> {
    if let Some(var) = path_var {
        for dir in std::env::split_paths(var) {
            let candidato = dir.join(NOME_EXE);
            if candidato.is_file() {
                return Some(candidato);
            }
        }
    }
    let pacchetti = local_app_data?.join("Microsoft").join("WinGet").join("Packages");
    let mut cartelle: Vec<PathBuf> = std::fs::read_dir(pacchetti)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with(PREFISSO_PACCHETTO_WINGET))
        .map(|e| e.path())
        .collect();
    cartelle.sort();
    cartelle
        .into_iter()
        .map(|c| c.join(NOME_EXE))
        .find(|p| p.is_file())
}

/// Percorso di `speedtest.exe` sul sistema, se installato.
pub fn trova_cli() -> Option<PathBuf> {
    trova_in(
        std::env::var_os("PATH").as_ref(),
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).as_deref(),
    )
}

pub fn cli_o_errore() -> Result<PathBuf, ErroreSpeedtest> {
    trova_cli().ok_or(ErroreSpeedtest::CliMancante)
}

/// Comando senza finestra console.
pub fn comando(exe: &Path) -> Command {
    let mut c = Command::new(exe);
    c.creation_flags(CREATE_NO_WINDOW);
    c
}

/// Flag con cui si accettano licenza e informativa di Ookla: si passano solo se l'utente le ha accettate.
pub const ARGOMENTI_LICENZA: [&str; 2] = ["--accept-license", "--accept-gdpr"];

/// Argomenti di `speedtest -L`; i flag di licenza solo se `licenza_accettata`.
pub fn argomenti_elenco(licenza_accettata: bool) -> Vec<String> {
    let mut a = vec!["-L".to_string(), "--format=json".to_string()];
    if licenza_accettata {
        a.extend(ARGOMENTI_LICENZA.iter().map(|s| s.to_string()));
    }
    a
}

/// `speedtest -L --format=json` (+ `--accept-license --accept-gdpr` se la licenza è accettata).
pub async fn elenca_server(exe: &Path, licenza_accettata: bool) -> Result<Vec<ServerSpeedtest>, ErroreSpeedtest> {
    super::controlla_licenza(licenza_accettata)?;
    let out = comando(exe)
        .args(argomenti_elenco(licenza_accettata))
        .stdin(Stdio::null())
        .output()
        .await
        .map_err(|e| ErroreSpeedtest::Fallito(tr!("Impossibile avviare speedtest: {e}", "Unable to start speedtest: {e}")))?;
    if !out.status.success() {
        return Err(ErroreSpeedtest::Fallito(tr!(
            "speedtest -L terminato con {}: {}",
            "speedtest -L exited with {}: {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    parse_elenco_server(&String::from_utf8_lossy(&out.stdout)).map_err(ErroreSpeedtest::Fallito)
}

/// Tempo massimo per l'installazione con winget.
pub const TIMEOUT_WINGET: Duration = Duration::from_secs(600);

/// Esegue un comando raccogliendone l'output, con un tempo massimo: allo scadere il processo
/// viene terminato (kill_on_drop) e si restituisce un errore.
pub async fn esegui_con_timeout(mut c: Command, limite: Duration, nome: &str) -> Result<String, String> {
    c.stdin(Stdio::null()).kill_on_drop(true);
    let out = match timeout(limite, c.output()).await {
        Err(_) => {
            return Err(tr!(
                "{nome} bloccato: interrotto dopo {}",
                "{nome} stuck: stopped after {}",
                super::esecuzione::descrivi_durata(limite)
            ));
        }
        Ok(Err(e)) => return Err(tr!("Impossibile avviare {nome}: {e}", "Unable to start {nome}: {e}")),
        Ok(Ok(out)) => out,
    };
    let testo = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        Ok(testo)
    } else {
        Err(tr!(
            "{nome} terminato con {}: {} {}",
            "{nome} exited with {}: {} {}",
            out.status,
            testo,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// Installa la CLI con winget (da chiamare solo su richiesta esplicita dell'utente), con un
/// tempo massimo di 10 minuti.
pub async fn installa_con_winget() -> Result<String, String> {
    let mut c = Command::new("winget");
    c.creation_flags(CREATE_NO_WINDOW);
    c.args([
        "install",
        "--id",
        "Ookla.Speedtest.CLI",
        "-e",
        "--accept-source-agreements",
        "--accept-package-agreements",
    ]);
    esegui_con_timeout(c, TIMEOUT_WINGET, "winget").await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn elenco_server_con_e_senza_licenza() {
        assert_eq!(argomenti_elenco(false), vec!["-L", "--format=json"]);
        assert_eq!(
            argomenti_elenco(true),
            vec!["-L", "--format=json", "--accept-license", "--accept-gdpr"]
        );
    }

    #[tokio::test]
    async fn elenco_server_rifiutato_senza_licenza() {
        // Il rifiuto avviene prima di toccare l'eseguibile (che qui non esiste)
        let r = elenca_server(Path::new("non-esiste.exe"), false).await;
        assert_eq!(r, Err(ErroreSpeedtest::LicenzaNonAccettata));
    }

    #[test]
    fn trovato_nel_path() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(bin.join(NOME_EXE), b"x").unwrap();
        let path_var = std::env::join_paths([dir.path().join("vuota"), bin.clone()]).unwrap();
        assert_eq!(trova_in(Some(&path_var), None), Some(bin.join(NOME_EXE)));
    }

    #[test]
    fn trovato_nei_pacchetti_winget() {
        let dir = tempfile::tempdir().unwrap();
        let pacchetto = dir
            .path()
            .join("Microsoft")
            .join("WinGet")
            .join("Packages")
            .join("Ookla.Speedtest.CLI_Microsoft.Winget.Source_8wekyb3d8bbwe");
        fs::create_dir_all(&pacchetto).unwrap();
        fs::write(pacchetto.join(NOME_EXE), b"x").unwrap();
        let altro = dir.path().join("Microsoft/WinGet/Packages/Altro.Pacchetto_x");
        fs::create_dir_all(&altro).unwrap();
        fs::write(altro.join(NOME_EXE), b"x").unwrap();
        let vuoto = std::env::join_paths([dir.path().join("niente")]).unwrap();
        assert_eq!(trova_in(Some(&vuoto), Some(dir.path())), Some(pacchetto.join(NOME_EXE)));
    }

    #[test]
    fn il_path_ha_la_precedenza_sui_pacchetti() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::write(bin.join(NOME_EXE), b"x").unwrap();
        let pacchetto = dir.path().join("Microsoft/WinGet/Packages/Ookla.Speedtest.CLI_x");
        fs::create_dir_all(&pacchetto).unwrap();
        fs::write(pacchetto.join(NOME_EXE), b"x").unwrap();
        let path_var = std::env::join_paths([bin.clone()]).unwrap();
        assert_eq!(trova_in(Some(&path_var), Some(dir.path())), Some(bin.join(NOME_EXE)));
    }

    #[test]
    fn mancante_se_non_c_e_da_nessuna_parte() {
        let dir = tempfile::tempdir().unwrap();
        let path_var = std::env::join_paths([dir.path()]).unwrap();
        assert_eq!(trova_in(Some(&path_var), Some(dir.path())), None);
        assert_eq!(trova_in(None, None), None);
    }

    #[tokio::test]
    async fn winget_bloccato_viene_interrotto_dopo_il_limite() {
        let dir = tempfile::tempdir().unwrap();
        let bat = dir.path().join("lento.bat");
        fs::write(&bat, "@echo off\r\nping -n 5 127.0.0.1 >nul\r\n").unwrap();
        let inizio = std::time::Instant::now();
        let r = esegui_con_timeout(Command::new(&bat), Duration::from_millis(600), "winget").await;
        assert_eq!(r, Err("winget bloccato: interrotto dopo 0 s".to_string()));
        assert!(inizio.elapsed() < Duration::from_secs(10));
        assert_eq!(TIMEOUT_WINGET, Duration::from_secs(600));
        assert_eq!(super::super::esecuzione::descrivi_durata(TIMEOUT_WINGET), "10 min");
    }

    #[tokio::test]
    async fn comando_che_termina_in_tempo() {
        let dir = tempfile::tempdir().unwrap();
        let bat = dir.path().join("ok.bat");
        fs::write(&bat, "@echo off\r\necho installato\r\n").unwrap();
        let r = esegui_con_timeout(Command::new(&bat), Duration::from_secs(20), "winget").await;
        assert_eq!(r, Ok("installato".to_string()));
        let bat2 = dir.path().join("ko.bat");
        fs::write(&bat2, "@echo off\r\nexit /b 3\r\n").unwrap();
        let r = esegui_con_timeout(Command::new(&bat2), Duration::from_secs(20), "winget").await;
        assert!(r.unwrap_err().starts_with("winget terminato con"));
    }
}
