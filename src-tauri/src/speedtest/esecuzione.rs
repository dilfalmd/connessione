//! Esecuzione di una misura con lo Speedtest CLI: lettura riga per riga, eventi di avanzamento,
//! annullamento (terminazione del processo).

use std::path::Path;
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::time::timeout;

use super::cli::comando;
use super::parsing::{EventoCli, RisultatoCli, parse_riga};
use super::tipi::{ErroreSpeedtest, ProgressoSpeedtest};
use crate::testi::tr;

/// Stato condiviso fra `esegui_speedtest` e `annulla_speedtest` (gestito da Tauri).
#[derive(Default)]
pub struct StatoSpeedtest {
    in_esecuzione: AtomicBool,
    annullato: AtomicBool,
    notifica: Notify,
}

/// Segna il test come in esecuzione; al drop torna libero.
pub struct PermessoEsecuzione<'a>(&'a StatoSpeedtest);

impl Drop for PermessoEsecuzione<'_> {
    fn drop(&mut self) {
        self.0.in_esecuzione.store(false, Ordering::SeqCst);
    }
}

impl StatoSpeedtest {
    pub fn acquisisci(&self) -> Result<PermessoEsecuzione<'_>, ErroreSpeedtest> {
        self.in_esecuzione
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ErroreSpeedtest::GiaInEsecuzione)?;
        self.annullato.store(false, Ordering::SeqCst);
        Ok(PermessoEsecuzione(self))
    }

    /// Chiede l'interruzione del test in corso (no-op se non ce n'è uno).
    pub fn annulla(&self) {
        if self.in_esecuzione.load(Ordering::SeqCst) {
            self.annullato.store(true, Ordering::SeqCst);
            self.notifica.notify_one();
        }
    }

    pub fn e_annullato(&self) -> bool {
        self.annullato.load(Ordering::SeqCst)
    }
}

/// Argomenti della CLI per una misura; i flag di licenza solo se `licenza_accettata`.
pub fn argomenti(server_id: Option<u32>, licenza_accettata: bool) -> Vec<String> {
    let mut a: Vec<String> = ["--format=jsonl", "--progress=yes"].iter().map(|s| s.to_string()).collect();
    if licenza_accettata {
        a.extend(super::cli::ARGOMENTI_LICENZA.iter().map(|s| s.to_string()));
    }
    if let Some(id) = server_id {
        a.push("-s".into());
        a.push(id.to_string());
    }
    a
}

/// Tempo massimo per una ripetizione: oltre, la CLI è considerata bloccata e viene terminata.
pub const TIMEOUT_RIPETIZIONE: Duration = Duration::from_secs(180);

/// «3 min» per i multipli di 60 secondi, altrimenti «N s».
pub fn descrivi_durata(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 60 && s.is_multiple_of(60) { format!("{} min", s / 60) } else { format!("{s} s") }
}

pub fn messaggio_timeout(limite: Duration) -> String {
    tr!("Speedtest bloccato: interrotto dopo {}", "Speed test stuck: stopped after {}", descrivi_durata(limite))
}

/// Esegue una misura. `emetti` riceve gli eventi di avanzamento.
pub async fn esegui_misura(
    exe: &Path,
    server_id: Option<u32>,
    ripetizione: u8,
    totale: u8,
    stato: &StatoSpeedtest,
    licenza_accettata: bool,
    emetti: impl Fn(ProgressoSpeedtest),
) -> Result<RisultatoCli, ErroreSpeedtest> {
    super::controlla_licenza(licenza_accettata)?;
    let mut c = comando(exe);
    c.args(argomenti(server_id, licenza_accettata));
    esegui_processo(c, ripetizione, totale, stato, TIMEOUT_RIPETIZIONE, emetti).await
}

enum Fine {
    Annullato,
    Terminato(std::io::Result<ExitStatus>),
}

/// Avvia il processo, ne legge l'output riga per riga e ne attende la fine entro `limite`
/// (lettura e `wait` compresi). Allo scadere o all'annullamento il processo viene terminato.
pub async fn esegui_processo(
    mut c: Command,
    ripetizione: u8,
    totale: u8,
    stato: &StatoSpeedtest,
    limite: Duration,
    emetti: impl Fn(ProgressoSpeedtest),
) -> Result<RisultatoCli, ErroreSpeedtest> {
    let mut child = c
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| ErroreSpeedtest::Fallito(tr!("Impossibile avviare speedtest: {e}", "Unable to start speedtest: {e}")))?;

    let stdout = child.stdout.take().expect("stdout rediretto");
    let mut stderr = child.stderr.take().expect("stderr rediretto");
    let task_stderr = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });

    let mut righe = BufReader::new(stdout).lines();
    let mut risultato: Option<RisultatoCli> = None;
    let mut errore_cli: Option<String> = None;

    let fine = timeout(limite, async {
        loop {
            tokio::select! {
                riga = righe.next_line() => {
                    match riga {
                        Ok(Some(riga)) => match parse_riga(&riga) {
                            Some(EventoCli::Progresso { fase, avanzamento, valore }) => emetti(ProgressoSpeedtest {
                                ripetizione, totale, fase, avanzamento, valore,
                            }),
                            Some(EventoCli::Risultato(r)) => risultato = Some(r),
                            Some(EventoCli::Errore(m)) => errore_cli = Some(m),
                            Some(EventoCli::Ignorato) | None => {}
                        },
                        Ok(None) | Err(_) => break,
                    }
                }
                _ = stato.notifica.notified() => {
                    if stato.e_annullato() {
                        let _ = child.kill().await;
                        return Fine::Annullato;
                    }
                }
            }
        }
        Fine::Terminato(child.wait().await)
    })
    .await;

    let esito = match fine {
        Err(_) => {
            // Scaduto il tempo: la CLI e` bloccata. Terminazione e rilascio sono garantiti:
            // il chiamante lascia cadere il permesso di esecuzione tornando con l'errore.
            let _ = child.kill().await;
            task_stderr.abort();
            return Err(ErroreSpeedtest::Fallito(messaggio_timeout(limite)));
        }
        Ok(Fine::Annullato) => {
            task_stderr.abort();
            return Err(ErroreSpeedtest::Annullato);
        }
        Ok(Fine::Terminato(esito)) => esito,
    };
    let stderr_testo = task_stderr.await.unwrap_or_default();
    if stato.e_annullato() {
        return Err(ErroreSpeedtest::Annullato);
    }
    if let Some(r) = risultato {
        return Ok(r);
    }
    let dettaglio = errore_cli
        .or_else(|| Some(stderr_testo.trim().to_string()).filter(|s| !s.is_empty()))
        .unwrap_or_else(|| match esito {
            Ok(s) => tr!("speedtest terminato con {s} senza risultato", "speedtest exited with {s} without a result"),
            Err(e) => tr!("speedtest terminato in modo anomalo: {e}", "speedtest ended abnormally: {e}"),
        });
    Err(ErroreSpeedtest::Fallito(dettaglio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argomenti_senza_e_con_server() {
        assert_eq!(
            argomenti(None, true),
            vec!["--format=jsonl", "--progress=yes", "--accept-license", "--accept-gdpr"]
        );
        let a = argomenti(Some(22857), true);
        assert_eq!(&a[4..], &["-s", "22857"]);
    }

    #[test]
    fn i_flag_di_licenza_si_passano_solo_se_accettata() {
        let senza = argomenti(Some(1), false);
        assert_eq!(senza, vec!["--format=jsonl", "--progress=yes", "-s", "1"]);
        assert!(!argomenti(None, false).iter().any(|a| a.starts_with("--accept")));
    }

    #[test]
    fn un_solo_test_alla_volta_e_annullamento() {
        let s = StatoSpeedtest::default();
        s.annulla(); // nessun test: non succede nulla
        assert!(!s.e_annullato());
        {
            let _p = s.acquisisci().unwrap();
            assert_eq!(s.acquisisci().err(), Some(ErroreSpeedtest::GiaInEsecuzione));
            s.annulla();
            assert!(s.e_annullato());
        }
        // Dopo il rilascio si può ripartire, e l'annullamento precedente è azzerato
        let _p = s.acquisisci().unwrap();
        assert!(!s.e_annullato());
    }

    use std::time::Instant;

    /// Crea un piccolo .bat di prova (nessuno speedtest reale).
    fn script(dir: &tempfile::TempDir, nome: &str, contenuto: &str) -> std::path::PathBuf {
        let p = dir.path().join(nome);
        std::fs::write(&p, format!("@echo off\r\n{contenuto}\r\n")).unwrap();
        p
    }

    const RISULTATO: &str = r#"echo {"type":"result","ping":{"latency":10.0,"jitter":1.0},"download":{"bandwidth":12500000},"upload":{"bandwidth":1250000}}"#;
    const PROGRESSO: &str = r#"echo {"type":"ping","ping":{"latency":10.0,"progress":0.5}}"#;

    #[test]
    fn messaggio_di_timeout_a_3_minuti() {
        assert_eq!(
            messaggio_timeout(TIMEOUT_RIPETIZIONE),
            "Speedtest bloccato: interrotto dopo 3 min"
        );
        assert_eq!(descrivi_durata(Duration::from_secs(90)), "90 s");
        assert_eq!(TIMEOUT_RIPETIZIONE, Duration::from_secs(180));
    }

    #[tokio::test]
    async fn processo_che_termina_con_risultato() {
        let dir = tempfile::tempdir().unwrap();
        let bat = script(&dir, "ok.bat", &format!("{PROGRESSO}\r\n{RISULTATO}"));
        let stato = StatoSpeedtest::default();
        let _p = stato.acquisisci().unwrap();
        let eventi = std::cell::Cell::new(0);
        let r = esegui_processo(Command::new(&bat), 1, 1, &stato, Duration::from_secs(20), |_| {
            eventi.set(eventi.get() + 1)
        })
        .await
        .unwrap();
        assert_eq!(r.download_mbps, 100.0);
        assert_eq!(r.upload_mbps, 10.0);
        assert_eq!(eventi.get(), 1);
    }

    #[tokio::test]
    async fn processo_bloccato_viene_interrotto_e_il_permesso_si_libera() {
        let dir = tempfile::tempdir().unwrap();
        let bat = script(&dir, "bloccato.bat", &format!("{PROGRESSO}\r\nping -n 5 127.0.0.1 >nul"));
        let stato = StatoSpeedtest::default();
        let inizio = Instant::now();
        {
            let _permesso = stato.acquisisci().unwrap();
            let r = esegui_processo(Command::new(&bat), 1, 1, &stato, Duration::from_millis(1500), |_| {}).await;
            match r {
                Err(ErroreSpeedtest::Fallito(m)) => {
                    assert_eq!(m, "Speedtest bloccato: interrotto dopo 1 s");
                }
                altro => panic!("atteso il timeout, ottenuto {altro:?}"),
            }
        }
        assert!(inizio.elapsed() < Duration::from_secs(10), "non deve aspettare la fine del processo");
        // Permesso rilasciato: si puo` ripartire
        assert!(stato.acquisisci().is_ok());
    }

    #[tokio::test]
    async fn annullamento_durante_l_esecuzione() {
        let dir = tempfile::tempdir().unwrap();
        let bat = script(&dir, "lungo.bat", "ping -n 5 127.0.0.1 >nul");
        let stato = StatoSpeedtest::default();
        let _p = stato.acquisisci().unwrap();
        let inizio = Instant::now();
        let (r, _) = tokio::join!(
            esegui_processo(Command::new(&bat), 1, 1, &stato, Duration::from_secs(60), |_| {}),
            async {
                tokio::time::sleep(Duration::from_millis(800)).await;
                stato.annulla();
            }
        );
        assert_eq!(r.err(), Some(ErroreSpeedtest::Annullato));
        assert!(inizio.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn processo_senza_risultato_e_un_errore() {
        let dir = tempfile::tempdir().unwrap();
        let bat = script(&dir, "vuoto.bat", "echo niente di utile");
        let stato = StatoSpeedtest::default();
        let _p = stato.acquisisci().unwrap();
        let r = esegui_processo(Command::new(&bat), 1, 1, &stato, Duration::from_secs(20), |_| {}).await;
        assert!(matches!(r, Err(ErroreSpeedtest::Fallito(_))));
    }
}
