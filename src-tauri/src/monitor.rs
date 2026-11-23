//! Monitoraggio in background: scheduler dei controlli automatici (anche a finestra nascosta),
//! impostazioni, notifiche e aggiornamento della tray dopo ogni controllo.

use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use tokio::sync::Notify;
use tokio::time::{Instant, sleep_until};

use crate::archivio::{Archivio, MisuraVelocita};
use crate::check::{self, FaseCheck};
use crate::consumo;
use crate::impostazioni::Impostazioni;
use crate::notifiche::Notificatore;
use crate::scanner::{self, SecurityReport};
use crate::speedtest::{self, StatoSpeedtest};
use crate::tray;
use crate::testi::{self, tr};

/// Dopo un avvio con `--minimizzato` il primo controllo parte presto, senza aspettare l'intervallo.
const PRIMO_CONTROLLO_MINIMIZZATO: Duration = Duration::from_secs(5);

/// Stato del monitoraggio (gestito da Tauri).
pub struct Monitor {
    impostazioni: Mutex<Impostazioni>,
    cambiate: Notify,
    /// Solo build di debug: intervallo di sicurezza in secondi che scavalca le impostazioni.
    debug_intervallo_sec: Mutex<Option<u64>>,
}

impl Monitor {
    pub fn new(imp: Impostazioni) -> Self {
        Monitor {
            impostazioni: Mutex::new(imp),
            cambiate: Notify::new(),
            debug_intervallo_sec: Mutex::new(None),
        }
    }

    pub fn corrente(&self) -> Impostazioni {
        self.impostazioni.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Imposta i nuovi valori e fa ripartire lo scheduler.
    pub fn imposta(&self, imp: Impostazioni) {
        *self.impostazioni.lock().unwrap_or_else(|e| e.into_inner()) = imp;
        self.cambiate.notify_one();
    }

    fn debug_intervallo(&self) -> Option<u64> {
        *self.debug_intervallo_sec.lock().unwrap_or_else(|e| e.into_inner())
    }
}

pub fn intervallo_sicurezza(imp: &Impostazioni, debug_sec: Option<u64>) -> Duration {
    match debug_sec {
        Some(s) => Duration::from_secs(s.max(1)),
        None => Duration::from_secs(u64::from(imp.intervallo_sicurezza_min) * 60),
    }
}

pub fn intervallo_speedtest(imp: &Impostazioni) -> Option<Duration> {
    imp.speedtest_ogni_ore.map(|h| Duration::from_secs(u64::from(h) * 3600))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuovoControllo {
    pub controllo_id: Option<i64>,
    pub report: SecurityReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct NuovaMisura {
    pub misure: Vec<MisuraVelocita>,
}

/// Da chiamare dopo ogni controllo di sicurezza salvato (manuale o automatico):
/// notifiche, tray ed (eventualmente) evento per la dashboard.
pub fn dopo_controllo(app: &AppHandle, report: &SecurityReport, id: Option<i64>, automatico: bool) {
    let imp = app.state::<Monitor>().corrente();
    app.state::<Notificatore>().nuovo_controllo(app, report, &imp);
    tray::aggiorna_da_report(app, report);
    if automatico {
        let _ = app.emit(
            "nuovo-controllo",
            NuovoControllo { controllo_id: id, report: report.clone() },
        );
    }
}

/// Da chiamare dopo ogni serie di misure di velocità salvate.
pub fn dopo_misure(app: &AppHandle, misure: &[MisuraVelocita], automatico: bool) {
    let imp = app.state::<Monitor>().corrente();
    let notificatore = app.state::<Notificatore>();
    for m in misure {
        notificatore.nuova_misura(app, m, &imp);
    }
    if automatico && !misure.is_empty() {
        let _ = app.emit("nuova-misura", NuovaMisura { misure: misure.to_vec() });
    }
}

/// Esito di un turno automatico.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Esito {
    Eseguito,
    /// Un check manuale (o un altro test) occupa il turno: si riprova presto.
    Occupato,
    /// Turno saltato di proposito (es. connessione a consumo): si aspetta il prossimo intervallo.
    Saltato,
}

/// Se il turno è occupato si riprova dopo 60 secondi invece di perderlo fino al prossimo intervallo.
const RIPROVA_SE_OCCUPATO: Duration = Duration::from_secs(60);

/// Tra quanto è il prossimo turno dopo un tentativo.
fn attesa_dopo(esito: Esito, intervallo: Duration) -> Duration {
    match esito {
        Esito::Occupato => RIPROVA_SE_OCCUPATO,
        Esito::Eseguito | Esito::Saltato => intervallo,
    }
}

/// Aggiorna le scadenze dopo un cambio di impostazioni: si azzera `prossima_sicurezza` solo se
/// cambiano il suo intervallo (o l'override di debug) o `monitoraggio_attivo`, e
/// `prossimo_speedtest` solo se cambia il suo intervallo o `monitoraggio_attivo`.
/// Altre modifiche (soglie, notifiche, tray...) lasciano i turni dove sono.
fn aggiorna_scadenze(
    prima: &Impostazioni,
    prima_debug: Option<u64>,
    dopo: &Impostazioni,
    dopo_debug: Option<u64>,
    ora: Instant,
    prossima_sicurezza: &mut Instant,
    prossimo_speedtest: &mut Option<Instant>,
) {
    let cambia_attivo = prima.monitoraggio_attivo != dopo.monitoraggio_attivo;
    if cambia_attivo
        || prima.intervallo_sicurezza_min != dopo.intervallo_sicurezza_min
        || prima_debug != dopo_debug
    {
        *prossima_sicurezza = ora + intervallo_sicurezza(dopo, dopo_debug);
    }
    if cambia_attivo || prima.speedtest_ogni_ore != dopo.speedtest_ogni_ore {
        *prossimo_speedtest = None; // si ricalcola all'inizio del ciclo
    }
}

async fn controllo_automatico(app: &AppHandle) -> Esito {
    let stato = app.state::<StatoSpeedtest>();
    let Ok(_permesso) = stato.acquisisci() else {
        return Esito::Occupato;
    };
    // La dashboard (se aperta) mostra il controllo in corso
    check::emetti(app, FaseCheck::Sicurezza);
    let esito = controllo_e_salva(app).await;
    check::emetti(app, FaseCheck::Fatto);
    esito
}

async fn controllo_e_salva(app: &AppHandle) -> Esito {
    let Ok(report) = scanner::esegui_scansione().await else {
        return Esito::Eseguito;
    };
    let id = app.state::<Archivio>().salva_se_valido(&report);
    dopo_controllo(app, &report, id, true);
    Esito::Eseguito
}

async fn speedtest_automatico(app: &AppHandle) -> Esito {
    let imp = app.state::<Monitor>().corrente();
    // Senza la licenza di Ookla accettata dall'utente non si lancia la CLI (i controlli di
    // sicurezza vanno avanti)
    if speedtest::controlla_licenza(imp.licenza_ookla_accettata).is_err() {
        eprintln!("Speedtest automatico saltato: licenza di Ookla non accettata");
        return Esito::Saltato;
    }
    // Mai su una connessione a consumo: lo speedtest automatico salta il turno
    if consumo::a_consumo(imp.modalita_consumo).await {
        eprintln!("Speedtest automatico saltato: connessione a consumo");
        return Esito::Saltato;
    }
    let stato = app.state::<StatoSpeedtest>();
    let Ok(_permesso) = stato.acquisisci() else {
        return Esito::Occupato;
    };
    let Ok(exe) = speedtest::trova_cli() else {
        return Esito::Saltato;
    };
    let archivio = app.state::<Archivio>();
    // Fase "velocita" attiva: la dashboard mostra l'avanzamento e il pulsante Annulla
    check::emetti(app, FaseCheck::Velocita);
    let esito = speedtest::serie(
        app,
        &stato,
        &archivio,
        &exe,
        speedtest::ParametriSerie {
            server_id: imp.server_speedtest,
            ripetizioni: imp.ripetizioni_speedtest,
            controllo_id: None,
            licenza_accettata: imp.licenza_ookla_accettata,
        },
    )
    .await;
    check::emetti(app, FaseCheck::Fatto);
    // Anche le misure parziali (serie fallita o annullata) passano da notifiche, soglie ed evento
    dopo_misure(app, &esito.misure, true);
    if let Some(e) = &esito.errore {
        eprintln!("Speedtest automatico interrotto: {e}");
    }
    Esito::Eseguito
}

/// Ciclo dello scheduler: gira per tutta la vita dell'app, indipendente dalla finestra.
pub async fn ciclo(app: AppHandle, avvio_minimizzato: bool) {
    let monitor = app.state::<Monitor>();
    let mut snap = monitor.corrente();
    let mut snap_debug = monitor.debug_intervallo();
    let mut prossima_sicurezza = Instant::now()
        + if avvio_minimizzato {
            PRIMO_CONTROLLO_MINIMIZZATO
        } else {
            intervallo_sicurezza(&snap, snap_debug)
        };
    let mut prossimo_speedtest: Option<Instant> = None;

    loop {
        let imp = monitor.corrente();
        if !imp.monitoraggio_attivo {
            // In pausa: si aspetta un cambio di impostazioni
            monitor.cambiate.notified().await;
            let nuove = monitor.corrente();
            let dbg = monitor.debug_intervallo();
            aggiorna_scadenze(&snap, snap_debug, &nuove, dbg, Instant::now(), &mut prossima_sicurezza, &mut prossimo_speedtest);
            snap = nuove;
            snap_debug = dbg;
            continue;
        }
        let int_sic = intervallo_sicurezza(&imp, monitor.debug_intervallo());
        let int_speed = intervallo_speedtest(&imp);
        match (int_speed, prossimo_speedtest) {
            (None, _) => prossimo_speedtest = None,
            (Some(i), None) => prossimo_speedtest = Some(Instant::now() + i),
            _ => {}
        }
        let prossimo = match prossimo_speedtest {
            Some(s) => s.min(prossima_sicurezza),
            None => prossima_sicurezza,
        };

        tokio::select! {
            _ = sleep_until(prossimo) => {
                let ora = Instant::now();
                if ora >= prossima_sicurezza {
                    let esito = controllo_automatico(&app).await;
                    prossima_sicurezza = Instant::now() + attesa_dopo(esito, int_sic);
                }
                if let (Some(s), Some(i)) = (prossimo_speedtest, int_speed)
                    && ora >= s
                {
                    let esito = speedtest_automatico(&app).await;
                    prossimo_speedtest = Some(Instant::now() + attesa_dopo(esito, i));
                }
            }
            _ = monitor.cambiate.notified() => {
                let nuove = monitor.corrente();
                let dbg = monitor.debug_intervallo();
                aggiorna_scadenze(&snap, snap_debug, &nuove, dbg, Instant::now(), &mut prossima_sicurezza, &mut prossimo_speedtest);
                snap = nuove;
                snap_debug = dbg;
            }
        }
    }
}

// --- Comandi ---------------------------------------------------------------------

#[tauri::command]
pub fn leggi_impostazioni(monitor: State<'_, Monitor>) -> Impostazioni {
    monitor.corrente()
}

/// Valida, salva e applica le impostazioni (scheduler, tray, avvio con Windows).
#[tauri::command]
pub fn salva_impostazioni(
    app: AppHandle,
    monitor: State<'_, Monitor>,
    archivio: State<'_, Archivio>,
    impostazioni: Impostazioni,
) -> Result<Impostazioni, String> {
    let precedenti = monitor.corrente();
    let mut nuove = impostazioni.validata();
    // La licenza di Ookla si accetta solo col comando dedicato, non dal salvataggio generico
    nuove.licenza_ookla_accettata = precedenti.licenza_ookla_accettata;
    if nuove.avvio_con_windows != precedenti.avvio_con_windows {
        let autostart = app.autolaunch();
        let esito = if nuove.avvio_con_windows { autostart.enable() } else { autostart.disable() };
        esito.map_err(|e| tr!("Avvio con Windows non modificabile: {e}", "Unable to change start with Windows: {e}"))?;
    }
    archivio.salva_impostazioni(&nuove)?;
    // La lingua vale subito: testi del Rust, menu e tooltip della tray
    testi::applica(nuove.lingua);
    tray::imposta_pausa(&app, nuove.monitoraggio_attivo);
    tray::aggiorna_lingua(&app, &archivio, nuove.monitoraggio_attivo);
    monitor.imposta(nuove.clone());
    Ok(nuove)
}

/// Registra che l'utente ha accettato licenza, termini e informativa privacy di Ookla.
#[tauri::command]
pub fn accetta_licenza_ookla(
    monitor: State<'_, Monitor>,
    archivio: State<'_, Archivio>,
) -> Result<Impostazioni, String> {
    let mut imp = monitor.corrente();
    imp.licenza_ookla_accettata = true;
    archivio.salva_impostazioni(&imp)?;
    *monitor.impostazioni.lock().unwrap_or_else(|e| e.into_inner()) = imp.clone();
    Ok(imp)
}

/// Memorizza server e ripetizioni scelti nella barra (usati dallo speedtest automatico).
#[tauri::command]
pub fn imposta_scelta_speedtest(
    monitor: State<'_, Monitor>,
    archivio: State<'_, Archivio>,
    server_id: Option<u32>,
    ripetizioni: u8,
) -> Result<(), String> {
    let mut imp = monitor.corrente();
    imp.server_speedtest = server_id;
    imp.ripetizioni_speedtest = ripetizioni;
    let imp = imp.validata();
    archivio.salva_impostazioni(&imp)?;
    // Nessun riavvio dello scheduler: i valori si leggono a ogni speedtest
    *monitor.impostazioni.lock().unwrap_or_else(|e| e.into_inner()) = imp;
    Ok(())
}

/// Pausa/Riprendi dal menu della tray.
pub fn alterna_pausa(app: &AppHandle) {
    let monitor = app.state::<Monitor>();
    let mut imp = monitor.corrente();
    imp.monitoraggio_attivo = !imp.monitoraggio_attivo;
    if let Err(e) = app.state::<Archivio>().salva_impostazioni(&imp) {
        eprintln!("Impostazioni non salvate: {e}");
    }
    tray::imposta_pausa(app, imp.monitoraggio_attivo);
    monitor.imposta(imp.clone());
    // La dashboard (se aperta) rilegge le impostazioni
    let _ = app.emit("impostazioni-cambiate", imp);
}

/// SOLO BUILD DI DEBUG: imposta l'intervallo del controllo di sicurezza in secondi (senza salvarlo).
#[cfg(debug_assertions)]
#[tauri::command]
pub fn debug_intervallo_sicurezza(monitor: State<'_, Monitor>, secondi: Option<u64>) -> Result<(), String> {
    *monitor.debug_intervallo_sec.lock().unwrap_or_else(|e| e.into_inner()) = secondi;
    monitor.cambiate.notify_one();
    Ok(())
}

/// SOLO BUILD DI DEBUG: mostra una notifica di prova (con clic che apre la finestra).
#[cfg(debug_assertions)]
#[tauri::command]
pub fn debug_notifica_prova(app: AppHandle) -> Result<(), String> {
    crate::notifiche::mostra(
        &app,
        "Connessione - notifica di prova",
        "Se leggi questo messaggio le notifiche funzionano. Un clic apre la finestra.",
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intervalli_dalle_impostazioni() {
        let mut imp = Impostazioni::default();
        assert_eq!(intervallo_sicurezza(&imp, None), Duration::from_secs(15 * 60));
        assert_eq!(intervallo_sicurezza(&imp, Some(60)), Duration::from_secs(60));
        assert_eq!(intervallo_sicurezza(&imp, Some(0)), Duration::from_secs(1));
        imp.intervallo_sicurezza_min = 5;
        assert_eq!(intervallo_sicurezza(&imp, None), Duration::from_secs(300));
        assert_eq!(intervallo_speedtest(&imp), None);
        imp.speedtest_ogni_ore = Some(6);
        assert_eq!(intervallo_speedtest(&imp), Some(Duration::from_secs(6 * 3600)));
    }

    #[test]
    fn il_monitor_restituisce_le_impostazioni_e_le_aggiorna() {
        let m = Monitor::new(Impostazioni::default());
        assert!(m.corrente().monitoraggio_attivo);
        m.imposta(Impostazioni { monitoraggio_attivo: false, ..Default::default() });
        assert!(!m.corrente().monitoraggio_attivo);
        assert_eq!(m.debug_intervallo(), None);
    }

    #[tokio::test]
    async fn il_cambio_di_impostazioni_sveglia_lo_scheduler() {
        let m = Monitor::new(Impostazioni::default());
        m.imposta(Impostazioni { intervallo_sicurezza_min: 5, ..Default::default() });
        // notify_one conserva il permesso: la notifica non va persa se nessuno stava aspettando
        tokio::time::timeout(Duration::from_millis(200), m.cambiate.notified())
            .await
            .expect("lo scheduler deve essere svegliato");
    }

    #[test]
    fn payload_eventi() {
        let j = serde_json::to_value(NuovoControllo {
            controllo_id: Some(4),
            report: SecurityReport::vuoto("t".into()),
        })
        .unwrap();
        assert_eq!(j["controlloId"], 4);
        assert_eq!(j["report"]["timestamp"], "t");
    }

    #[test]
    fn turno_occupato_si_riprova_dopo_60_secondi() {
        let quindici_min = Duration::from_secs(15 * 60);
        assert_eq!(attesa_dopo(Esito::Occupato, quindici_min), Duration::from_secs(60));
        assert_eq!(attesa_dopo(Esito::Eseguito, quindici_min), quindici_min);
        assert_eq!(attesa_dopo(Esito::Saltato, quindici_min), quindici_min);
    }

    fn scadenze() -> (Instant, Instant, Option<Instant>) {
        let ora = Instant::now();
        (ora, ora + Duration::from_secs(100), Some(ora + Duration::from_secs(200)))
    }

    #[test]
    fn cambi_non_legati_agli_intervalli_non_azzerano_le_scadenze() {
        let (ora, sic0, speed0) = scadenze();
        let (mut sic, mut speed) = (sic0, speed0);
        let prima = Impostazioni { speedtest_ogni_ore: Some(6), ..Default::default() };
        let dopo = Impostazioni {
            soglia_download_min_mbps: Some(50.0),
            soglia_ping_max_ms: Some(80.0),
            notifiche_attive: false,
            chiudi_in_tray: false,
            server_speedtest: Some(7),
            ..prima.clone()
        };
        aggiorna_scadenze(&prima, None, &dopo, None, ora + Duration::from_secs(10), &mut sic, &mut speed);
        assert_eq!((sic, speed), (sic0, speed0));
    }

    #[test]
    fn cambio_dell_intervallo_di_sicurezza_azzera_solo_quello() {
        let (ora, sic0, speed0) = scadenze();
        let (mut sic, mut speed) = (sic0, speed0);
        let prima = Impostazioni::default();
        let dopo = Impostazioni { intervallo_sicurezza_min: 5, ..prima.clone() };
        aggiorna_scadenze(&prima, None, &dopo, None, ora, &mut sic, &mut speed);
        assert_eq!(sic, ora + Duration::from_secs(300));
        assert_eq!(speed, speed0);
    }

    #[test]
    fn cambio_dell_intervallo_speedtest_azzera_solo_quello() {
        let (ora, sic0, speed0) = scadenze();
        let (mut sic, mut speed) = (sic0, speed0);
        let prima = Impostazioni { speedtest_ogni_ore: Some(6), ..Default::default() };
        let dopo = Impostazioni { speedtest_ogni_ore: Some(1), ..prima.clone() };
        aggiorna_scadenze(&prima, None, &dopo, None, ora, &mut sic, &mut speed);
        assert_eq!(sic, sic0);
        assert_eq!(speed, None);
    }

    #[test]
    fn pausa_e_ripresa_azzerano_entrambe_e_il_debug_conta() {
        let (ora, sic0, speed0) = scadenze();
        let (mut sic, mut speed) = (sic0, speed0);
        let attivo = Impostazioni::default();
        let in_pausa = Impostazioni { monitoraggio_attivo: false, ..attivo.clone() };
        aggiorna_scadenze(&in_pausa, None, &attivo, None, ora, &mut sic, &mut speed);
        assert_eq!(sic, ora + Duration::from_secs(900));
        assert_eq!(speed, None);

        let (mut sic, mut speed) = (sic0, speed0);
        aggiorna_scadenze(&attivo, None, &attivo, Some(60), ora, &mut sic, &mut speed);
        assert_eq!(sic, ora + Duration::from_secs(60), "l'override di debug riavvia il turno di sicurezza");
        assert_eq!(speed, speed0);
    }
}
