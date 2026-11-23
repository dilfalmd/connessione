//! Notifiche di sistema (toast di Windows). Il clic sulla notifica porta in primo piano la finestra.
//!
//! Nota: il plugin `tauri-plugin-notification` su desktop non restituisce il clic sul toast, per
//! questo si usa direttamente `tauri-winrt-notification` (la libreria che il plugin stesso usa su
//! Windows), che permette `on_activated`.

pub mod logica;

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::AppHandle;
use tauri_winrt_notification::Toast;

use crate::archivio::MisuraVelocita;
use crate::impostazioni::Impostazioni;
use crate::scanner::SecurityReport;
use logica::{Avviso, Soglie, StatoNotifiche};

/// Stato condiviso delle notifiche (gestito da Tauri).
#[derive(Default)]
pub struct Notificatore {
    stato: Mutex<StatoNotifiche>,
}

fn adesso() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl Notificatore {
    /// Valuta un nuovo controllo e invia le notifiche se sono attive.
    pub fn nuovo_controllo(&self, app: &AppHandle, report: &SecurityReport, imp: &Impostazioni) {
        let avvisi = {
            let mut stato = self.stato.lock().unwrap_or_else(|e| e.into_inner());
            // Con le notifiche spente gli avvisi si scartano e non risultano "inviati"
            stato.silenziate = !imp.notifiche_attive;
            stato.controllo(report, adesso())
        };
        invia(app, &avvisi);
    }

    /// Valuta una nuova misura di velocità e invia le notifiche se sono attive.
    pub fn nuova_misura(&self, app: &AppHandle, misura: &MisuraVelocita, imp: &Impostazioni) {
        let soglie = Soglie {
            download_min_mbps: imp.soglia_download_min_mbps,
            ping_max_ms: imp.soglia_ping_max_ms,
        };
        let avvisi = {
            let mut stato = self.stato.lock().unwrap_or_else(|e| e.into_inner());
            stato.silenziate = !imp.notifiche_attive;
            stato.misura(misura, soglie, adesso())
        };
        invia(app, &avvisi);
    }
}

pub fn invia(app: &AppHandle, avvisi: &[Avviso]) {
    for a in avvisi {
        mostra(app, &a.titolo, &a.testo);
    }
}

/// Mostra un toast; il clic apre la finestra principale.
pub fn mostra(app: &AppHandle, titolo: &str, testo: &str) {
    // In sviluppo l'app non e` registrata: si usa l'identificatore di PowerShell, come fa il plugin.
    let app_id = if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    };
    let handle = app.clone();
    let esito = Toast::new(&app_id)
        .title(titolo)
        .text1(testo)
        .on_activated(move |_| {
            crate::tray::mostra_finestra(&handle);
            Ok(())
        })
        .show();
    if let Err(e) = esito {
        eprintln!("Notifica non mostrata: {e}");
    }
}
