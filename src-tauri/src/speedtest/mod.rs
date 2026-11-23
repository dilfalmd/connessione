//! Speedtest tramite la CLI di Ookla: elenco server, misure ripetute con avanzamento,
//! annullamento, salvataggio nell'archivio.

mod cli;
mod esecuzione;
mod parsing;
mod tipi;

use std::path::{Path, PathBuf};

use chrono::{Local, SecondsFormat};
use tauri::{AppHandle, Emitter, State};

use crate::archivio::{Archivio, MisuraVelocita};
use crate::monitor::Monitor;
use crate::scanner::rete_locale;

pub use esecuzione::StatoSpeedtest;
pub use tipi::{ErroreSpeedtest, RisultatoSpeedtest};
use parsing::{RisultatoCli, media_misure};
use tipi::ServerSpeedtest;

pub const EVENTO_PROGRESSO: &str = "speedtest-progresso";
const RIPETIZIONI_MAX: u8 = 5;

/// Rifiuta lo speedtest se l'utente non ha accettato la licenza di Ookla. È l'unico punto in cui
/// la regola è decisa: la usano i comandi, il check completo, il monitor e la costruzione dei comandi CLI.
pub fn controlla_licenza(accettata: bool) -> Result<(), ErroreSpeedtest> {
    if accettata { Ok(()) } else { Err(ErroreSpeedtest::LicenzaNonAccettata) }
}

#[cfg(test)]
mod test_ripetizioni {
    use super::*;

    #[test]
    fn licenza_richiesta() {
        assert_eq!(controlla_licenza(false), Err(ErroreSpeedtest::LicenzaNonAccettata));
        assert_eq!(controlla_licenza(true), Ok(()));
    }

    #[test]
    fn ripetizioni_da_1_a_5() {
        assert!(controlla_ripetizioni(0).is_err());
        assert!(controlla_ripetizioni(1).is_ok());
        assert!(controlla_ripetizioni(5).is_ok());
        assert!(controlla_ripetizioni(6).is_err());
    }
}

/// Rilevamento rapido di un tunnel VPN locale (solo interfacce e rotte, nessuna rete).
async fn vpn_attiva() -> bool {
    !rete_locale::ispeziona_rete_locale_async().await.tunnels.is_empty()
}

fn misura_da_cli(r: RisultatoCli, sessione: &str, vpn: bool) -> MisuraVelocita {
    MisuraVelocita {
        id: None,
        timestamp: Local::now().to_rfc3339_opts(SecondsFormat::Secs, false),
        sessione: sessione.to_string(),
        isp: r.isp,
        ip_esterno: r.ip_esterno,
        server_id: r.server_id.map(i64::from),
        server_nome: r.server_nome,
        server_luogo: r.server_luogo,
        ping_ms: r.ping_ms,
        jitter_ms: r.jitter_ms,
        download_mbps: r.download_mbps,
        upload_mbps: r.upload_mbps,
        packet_loss: r.packet_loss,
        url: r.url,
        vpn_attiva: Some(vpn),
        controllo_id: None,
        origine: "app".into(),
    }
}

/// Elenco dei server vicini (`speedtest -L`).
#[tauri::command]
pub async fn elenca_server(monitor: State<'_, Monitor>) -> Result<Vec<ServerSpeedtest>, ErroreSpeedtest> {
    let licenza = monitor.corrente().licenza_ookla_accettata;
    controlla_licenza(licenza)?;
    let exe = cli::cli_o_errore()?;
    cli::elenca_server(&exe, licenza).await
}

/// Vero se la CLI è installata (per decidere se mostrare «Installa con winget»).
#[tauri::command]
pub fn cli_speedtest_presente() -> bool {
    cli::trova_cli().is_some()
}

/// Installa la CLI con winget. Va chiamato solo dopo il clic esplicito dell'utente.
#[tauri::command]
pub async fn installa_speedtest_cli() -> Result<String, String> {
    cli::installa_con_winget().await
}

/// Esito di una serie di misure: quelle già salvate restano valide anche se la serie si
/// interrompe a metà (errore o annullamento) e vanno comunque notificate e mostrate.
pub(crate) struct EsitoSerie {
    pub sessione: String,
    pub misure: Vec<MisuraVelocita>,
    pub errore: Option<ErroreSpeedtest>,
}

impl EsitoSerie {
    /// Risultato con la media se la serie è completa, altrimenti l'errore.
    pub fn in_risultato(self) -> Result<RisultatoSpeedtest, ErroreSpeedtest> {
        if let Some(e) = self.errore {
            return Err(e);
        }
        let media = media_misure(&self.misure)
            .ok_or_else(|| ErroreSpeedtest::Fallito("Nessuna misura completata".into()))?;
        Ok(RisultatoSpeedtest { sessione: self.sessione, misure: self.misure, media })
    }
}

/// Parametri di una serie di misure.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ParametriSerie {
    pub server_id: Option<u32>,
    pub ripetizioni: u8,
    /// Controllo di sicurezza a cui collegare le misure (check completo).
    pub controllo_id: Option<i64>,
    /// Licenza di Ookla accettata dall'utente: senza, la serie non parte.
    pub licenza_accettata: bool,
}

/// Esegue `ripetizioni` misure consecutive, salvandole una a una nell'archivio e collegandole
/// (se presente) a `controllo_id`. Il chiamante deve aver già acquisito il permesso di esecuzione.
/// Se una ripetizione fallisce o viene annullata, le misure già fatte restano in `misure`.
pub(crate) async fn serie(
    app: &AppHandle,
    stato: &StatoSpeedtest,
    archivio: &Archivio,
    exe: &Path,
    parametri: ParametriSerie,
) -> EsitoSerie {
    let ParametriSerie { server_id, ripetizioni, controllo_id, licenza_accettata } = parametri;
    let sessione = uuid::Uuid::new_v4().to_string();
    let mut misure: Vec<MisuraVelocita> = Vec::new();
    let mut errore = controlla_licenza(licenza_accettata).err();
    for ripetizione in 1..=ripetizioni {
        if errore.is_some() {
            break;
        }
        if stato.e_annullato() {
            errore = Some(ErroreSpeedtest::Annullato);
            break;
        }
        let vpn = vpn_attiva().await;
        let r = match esecuzione::esegui_misura(exe, server_id, ripetizione, ripetizioni, stato, licenza_accettata, |p| {
            let _ = app.emit(EVENTO_PROGRESSO, p);
        })
        .await
        {
            Ok(r) => r,
            Err(e) => {
                errore = Some(e);
                break;
            }
        };
        let mut misura = misura_da_cli(r, &sessione, vpn);
        misura.controllo_id = controllo_id;
        match archivio.inserisci_misura(&misura) {
            Ok(id) => misura.id = Some(id),
            Err(e) => {
                errore = Some(ErroreSpeedtest::Fallito(e));
                break;
            }
        }
        misure.push(misura);
    }
    EsitoSerie { sessione, misure, errore }
}

pub(crate) fn controlla_ripetizioni(ripetizioni: u8) -> Result<(), ErroreSpeedtest> {
    if (1..=RIPETIZIONI_MAX).contains(&ripetizioni) {
        Ok(())
    } else {
        Err(ErroreSpeedtest::ArgomentoNonValido(format!(
            "le ripetizioni devono essere da 1 a {RIPETIZIONI_MAX}"
        )))
    }
}

pub(crate) fn trova_cli() -> Result<PathBuf, ErroreSpeedtest> {
    cli::cli_o_errore()
}

/// Esegue `ripetizioni` (1-5) misure consecutive e le salva nell'archivio.
/// Emette l'evento `speedtest-progresso` durante il test.
#[tauri::command]
pub async fn esegui_speedtest(
    app: AppHandle,
    stato: State<'_, StatoSpeedtest>,
    archivio: State<'_, Archivio>,
    monitor: State<'_, Monitor>,
    server_id: Option<u32>,
    ripetizioni: u8,
) -> Result<RisultatoSpeedtest, ErroreSpeedtest> {
    let licenza = monitor.corrente().licenza_ookla_accettata;
    controlla_licenza(licenza)?;
    controlla_ripetizioni(ripetizioni)?;
    let exe = cli::cli_o_errore()?;
    let _permesso = stato.acquisisci()?;
    let parametri = ParametriSerie { server_id, ripetizioni, controllo_id: None, licenza_accettata: licenza };
    let esito = serie(&app, &stato, &archivio, &exe, parametri).await;
    // Anche le misure parziali (serie fallita o annullata) passano da notifiche e soglie
    crate::monitor::dopo_misure(&app, &esito.misure, false);
    esito.in_risultato()
}

/// Interrompe il test in corso terminando il processo della CLI.
#[tauri::command]
pub fn annulla_speedtest(stato: State<'_, StatoSpeedtest>) {
    stato.annulla();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn misura_da_risultato_cli() {
        let r = RisultatoCli {
            ping_ms: 20.0,
            jitter_ms: Some(1.0),
            download_mbps: 100.0,
            upload_mbps: 10.0,
            packet_loss: None,
            isp: Some("Isp".into()),
            ip_esterno: Some("203.0.113.7".into()),
            server_id: Some(22857),
            server_nome: Some("Server Esempio".into()),
            server_luogo: Some("Milano".into()),
            url: Some("https://x/1".into()),
        };
        let m = misura_da_cli(r, "sess", true);
        assert_eq!(m.sessione, "sess");
        assert_eq!(m.server_id, Some(22857));
        assert_eq!(m.vpn_attiva, Some(true));
        assert_eq!(m.origine, "app");
        assert!(m.id.is_none() && m.packet_loss.is_none());
        assert!(m.timestamp.len() > 19);
    }

    fn misura_finta(down: f64) -> MisuraVelocita {
        misura_da_cli(
            RisultatoCli {
                ping_ms: 10.0,
                jitter_ms: None,
                download_mbps: down,
                upload_mbps: 1.0,
                packet_loss: None,
                isp: None,
                ip_esterno: None,
                server_id: None,
                server_nome: None,
                server_luogo: None,
                url: None,
            },
            "s",
            false,
        )
    }

    #[test]
    fn serie_interrotta_conserva_le_misure_gia_fatte() {
        let esito = EsitoSerie {
            sessione: "s".into(),
            misure: vec![misura_finta(100.0), misura_finta(200.0)],
            errore: Some(ErroreSpeedtest::Annullato),
        };
        assert_eq!(esito.misure.len(), 2, "le misure parziali restano disponibili per dopo_misure");
        assert_eq!(esito.in_risultato().err(), Some(ErroreSpeedtest::Annullato));
    }

    #[test]
    fn serie_completa_da_la_media() {
        let esito = EsitoSerie {
            sessione: "s".into(),
            misure: vec![misura_finta(100.0), misura_finta(200.0)],
            errore: None,
        };
        let r = esito.in_risultato().unwrap();
        assert_eq!(r.media.download_mbps, 150.0);
        let vuota = EsitoSerie { sessione: "s".into(), misure: vec![], errore: None };
        assert!(vuota.in_risultato().is_err());
    }
}
