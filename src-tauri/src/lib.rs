mod archivio;
mod check;
mod consumo;
mod dati_vecchi;
mod impostazioni;
mod monitor;
mod notifiche;
mod scanner;
mod speedtest;
mod testi;
mod tray;

use std::path::PathBuf;

use archivio::{Archivio, ControlloSicurezza, GruppoVpn, MisuraVelocita};
use monitor::Monitor;
use notifiche::Notificatore;
use scanner::SecurityReport;
use scanner::report::OverallStatus;
use speedtest::StatoSpeedtest;
use tauri::{AppHandle, Manager, State, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;

/// Argomento con cui l'avvio automatico di Windows fa partire l'app già nella tray.
const ARGOMENTO_MINIMIZZATO: &str = "--minimizzato";

/// Esegue il controllo di sicurezza completo della connessione/VPN e ne salva il report.
#[tauri::command]
async fn scan_sicurezza(
    app: AppHandle,
    archivio: State<'_, Archivio>,
    stato: State<'_, StatoSpeedtest>,
) -> Result<SecurityReport, String> {
    // Occupa lo stesso "turno" dei test: un controllo automatico concorrente salta
    let _permesso = stato.acquisisci().map_err(|e| e.to_string())?;
    let report = scanner::esegui_scansione().await?;
    let id = archivio.salva_se_valido(&report);
    monitor::dopo_controllo(&app, &report, id, false);
    Ok(report)
}

/// Misure di velocità fra `da` e `a` (ISO 8601, facoltativi), in ordine cronologico.
#[tauri::command]
fn storico_velocita(
    archivio: State<'_, Archivio>,
    da: Option<String>,
    a: Option<String>,
) -> Result<Vec<MisuraVelocita>, String> {
    archivio.storico_velocita(da.as_deref(), a.as_deref())
}

/// Controlli di sicurezza fra `da` e `a` (ISO 8601, facoltativi), in ordine cronologico.
#[tauri::command]
fn storico_sicurezza(
    archivio: State<'_, Archivio>,
    da: Option<String>,
    a: Option<String>,
) -> Result<Vec<ControlloSicurezza>, String> {
    archivio.storico_sicurezza(da.as_deref(), a.as_deref())
}

/// Gli ultimi `limite` controlli di sicurezza, dal più recente.
#[tauri::command]
fn ultimi_controlli(archivio: State<'_, Archivio>, limite: u32) -> Result<Vec<ControlloSicurezza>, String> {
    archivio.ultimi_controlli(limite)
}

/// Media, mediana e numero di misure di download, upload e ping per stato VPN
/// (acceso, spento, sconosciuto) nell'intervallo dato.
#[tauri::command]
fn confronto_vpn(
    archivio: State<'_, Archivio>,
    da: Option<String>,
    a: Option<String>,
) -> Result<Vec<GruppoVpn>, String> {
    archivio.confronto_vpn(da.as_deref(), a.as_deref())
}

/// Scrive un report .txt in italiano del controllo `controllo_id` (l'ultimo se assente)
/// con le misure di velocità collegate.
#[tauri::command]
fn esporta_report(
    archivio: State<'_, Archivio>,
    percorso: String,
    controllo_id: Option<i64>,
) -> Result<(), String> {
    archivio.esporta_report(&PathBuf::from(percorso), controllo_id)
}

/// Lingua in uso («it» o «en»): la scelta «automatica» è già risolta dal Rust.
#[tauri::command]
fn lingua_effettiva() -> &'static str {
    testi::corrente().codice()
}

/// Importa un CSV di storico (formato dello script PowerShell `speedtest_log.csv`) scelto dall'utente.
/// Una sola transazione; le righe già presenti si saltano. Restituisce il messaggio per l'utente.
#[tauri::command]
fn importa_storico_csv(archivio: State<'_, Archivio>, percorso: String) -> Result<String, String> {
    let percorso = PathBuf::from(percorso);
    controlla_estensione_csv(&percorso)?;
    let esito = archivio.importa_csv(&percorso)?;
    Ok(esito.messaggio())
}

/// Il comando può essere chiamato da script della pagina: si importano solo file .csv (senza badare alle maiuscole).
fn controlla_estensione_csv(percorso: &std::path::Path) -> Result<(), String> {
    let ok = percorso
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if ok {
        Ok(())
    } else {
        Err(testi::tr!(
            "Lo storico si può importare solo da un file con estensione .csv",
            "The history can only be imported from a file with a .csv extension"
        ))
    }
}

/// Stato iniziale di tray (icona e tooltip) dall'ultimo controllo salvato.
pub(crate) fn tray_iniziale(app: &AppHandle, archivio: &Archivio) {
    let Ok(ultimi) = archivio.ultimi_controlli(1) else { return };
    let Some(c) = ultimi.first() else { return };
    let stato = serde_json::from_value::<OverallStatus>(serde_json::Value::String(c.verdetto.clone())).ok();
    tray::aggiorna(app, stato, c.vpn_attiva, tray::ora_da_iso(&c.timestamp).as_deref());
}

/// Comandi esposti alla pagina. I comandi di debug (notifica di prova, intervallo di 60 s)
/// esistono e sono registrati solo nelle build di debug: nella release non sono compilati.
#[cfg(debug_assertions)]
macro_rules! gestori_comandi {
    () => {
        tauri::generate_handler![
            scan_sicurezza,
            storico_velocita,
            storico_sicurezza,
            ultimi_controlli,
            confronto_vpn,
            esporta_report,
            importa_storico_csv,
            lingua_effettiva,
            consumo::connessione_a_consumo,
            check::check_completo,
            monitor::leggi_impostazioni,
            monitor::salva_impostazioni,
            monitor::accetta_licenza_ookla,
            monitor::imposta_scelta_speedtest,
            speedtest::elenca_server,
            speedtest::cli_speedtest_presente,
            speedtest::installa_speedtest_cli,
            speedtest::esegui_speedtest,
            speedtest::annulla_speedtest,
            monitor::debug_intervallo_sicurezza,
            monitor::debug_notifica_prova,
        ]
    };
}

#[cfg(not(debug_assertions))]
macro_rules! gestori_comandi {
    () => {
        tauri::generate_handler![
            scan_sicurezza,
            storico_velocita,
            storico_sicurezza,
            ultimi_controlli,
            confronto_vpn,
            esporta_report,
            importa_storico_csv,
            lingua_effettiva,
            consumo::connessione_a_consumo,
            check::check_completo,
            monitor::leggi_impostazioni,
            monitor::salva_impostazioni,
            monitor::accetta_licenza_ookla,
            monitor::imposta_scelta_speedtest,
            speedtest::elenca_server,
            speedtest::cli_speedtest_presente,
            speedtest::installa_speedtest_cli,
            speedtest::esegui_speedtest,
            speedtest::annulla_speedtest,
        ]
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Istanza unica: un secondo avvio porta in primo piano la finestra già aperta
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::mostra_finestra(app);
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // Aggiornamenti da GitHub Releases: il controllo e l'installazione partono solo dalla pagina
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec![ARGOMENTO_MINIMIZZATO]),
        ))
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            // Dal vecchio identificatore (local.connessione) al nuovo: i dati si copiano una volta sola
            if let Some(base) = dir.parent() {
                match dati_vecchi::copia_dati_vecchi(&dir, &base.join(dati_vecchi::CARTELLA_VECCHIA)) {
                    Ok(true) => eprintln!("Dati copiati dalla vecchia cartella {}", dati_vecchi::CARTELLA_VECCHIA),
                    Ok(false) => {}
                    Err(e) => eprintln!("Copia dei dati dalla vecchia cartella non riuscita: {e}"),
                }
            }
            std::fs::create_dir_all(&dir)?;
            let archivio = Archivio::apri(&dir.join("connessione.db"))?;
            let imp = archivio.leggi_impostazioni().unwrap_or_default();
            // La lingua dei testi del Rust (tray, notifiche, verdetti) vale già dalla prima tray
            testi::applica(imp.lingua);
            let handle = app.handle().clone();

            tray::crea(&handle, imp.monitoraggio_attivo)?;
            tray_iniziale(&handle, &archivio);

            app.manage(archivio);
            app.manage(StatoSpeedtest::default());
            app.manage(Notificatore::default());
            app.manage(Monitor::new(imp));

            // La finestra nasce nascosta: si mostra subito, salvo avvio con --minimizzato
            let minimizzato = std::env::args().any(|a| a == ARGOMENTO_MINIMIZZATO);
            if !minimizzato {
                tray::mostra_finestra(&handle);
            }
            tauri::async_runtime::spawn(monitor::ciclo(handle, minimizzato));
            Ok(())
        })
        .on_window_event(|window, evento| {
            // Chiudendo la finestra l'app resta nella tray (se l'impostazione è attiva)
            if let WindowEvent::CloseRequested { api, .. } = evento
                && let Some(monitor) = window.app_handle().try_state::<Monitor>()
                && monitor.corrente().chiudi_in_tray
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(gestori_comandi!())
        .run(tauri::generate_context!())
        .expect("errore durante l'avvio dell'applicazione Tauri");
}

#[cfg(test)]
mod test {
    use super::controlla_estensione_csv;
    use std::path::Path;

    #[test]
    fn importa_csv_accetta_solo_estensione_csv() {
        assert!(controlla_estensione_csv(Path::new("storico.csv")).is_ok());
        assert!(controlla_estensione_csv(Path::new(r"C:\dati\STORICO.CSV")).is_ok());
        assert!(controlla_estensione_csv(Path::new("storico.Csv")).is_ok());
        for no in ["storico.txt", "storico.exe", "storico", "storico.csv.exe", ".csv", ""] {
            assert!(controlla_estensione_csv(Path::new(no)).is_err(), "{no}");
        }
    }
}
