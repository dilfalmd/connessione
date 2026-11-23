//! Icona nella tray: colore secondo l'ultimo verdetto, tooltip, menu e clic sinistro.

use chrono::DateTime;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::scanner::report::OverallStatus;
use crate::scanner::SecurityReport;
use crate::testi::{sc, tr};

const ID_TRAY: &str = "principale";
const ICONA_VERDE: &[u8] = include_bytes!("../icons/tray/tray-verde.png");
const ICONA_AMBRA: &[u8] = include_bytes!("../icons/tray/tray-ambra.png");
const ICONA_ROSSA: &[u8] = include_bytes!("../icons/tray/tray-rosso.png");
const ICONA_GRIGIA: &[u8] = include_bytes!("../icons/tray/tray-grigio.png");

/// Voce di menu «Pausa/Riprendi», tenuta per aggiornarne il testo.
pub struct VociTray {
    apri: MenuItem<Wry>,
    check: MenuItem<Wry>,
    sicurezza: MenuItem<Wry>,
    pausa: MenuItem<Wry>,
    esci: MenuItem<Wry>,
}

pub fn icona_per(stato: Option<OverallStatus>) -> &'static [u8] {
    match stato {
        Some(OverallStatus::Protected) => ICONA_VERDE,
        Some(OverallStatus::Warning) => ICONA_AMBRA,
        Some(OverallStatus::Unprotected) => ICONA_ROSSA,
        Some(OverallStatus::Error) | None => ICONA_GRIGIA,
    }
}

/// «Connessione — Protetta, VPN accesa, ultimo controllo 10:54»
pub fn testo_tooltip(stato: Option<OverallStatus>, vpn: Option<bool>, ora: Option<&str>) -> String {
    let verdetto = match stato {
        Some(OverallStatus::Error) | None => sc("verdetto sconosciuto", "verdict unknown"),
        Some(s) => crate::notifiche::logica::etichetta_verdetto(s),
    };
    let vpn = match vpn {
        Some(true) => sc("VPN accesa", "VPN on"),
        Some(false) => sc("VPN spenta", "VPN off"),
        None => sc("VPN sconosciuta", "VPN unknown"),
    };
    match ora {
        Some(o) => tr!(
            "Connessione — {verdetto}, {vpn}, ultimo controllo {o}",
            "Connessione — {verdetto}, {vpn}, last check {o}"
        ),
        None => tr!(
            "Connessione — {verdetto}, {vpn}, nessun controllo",
            "Connessione — {verdetto}, {vpn}, no check yet"
        ),
    }
}

/// Ora locale HH:MM da un timestamp ISO 8601.
pub fn ora_da_iso(iso: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(iso).ok().map(|d| d.format("%H:%M").to_string())
}

pub fn etichetta_pausa(monitoraggio_attivo: bool) -> &'static str {
    if monitoraggio_attivo { sc("Pausa monitoraggio", "Pause monitoring") } else { sc("Riprendi monitoraggio", "Resume monitoring") }
}

/// Porta in primo piano la finestra principale (anche se nascosta o ridotta a icona).
pub fn mostra_finestra(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

pub fn crea(app: &AppHandle, monitoraggio_attivo: bool) -> tauri::Result<()> {
    let apri = MenuItem::with_id(app, "apri", sc("Apri", "Open"), true, None::<&str>)?;
    let check = MenuItem::with_id(app, "check", sc("Check completo", "Full check"), true, None::<&str>)?;
    let sicurezza = MenuItem::with_id(app, "sicurezza", sc("Solo sicurezza", "Security only"), true, None::<&str>)?;
    let pausa = MenuItem::with_id(app, "pausa", etichetta_pausa(monitoraggio_attivo), true, None::<&str>)?;
    let esci = MenuItem::with_id(app, "esci", sc("Esci", "Quit"), true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&apri, &sep1, &check, &sicurezza, &pausa, &sep2, &esci])?;

    TrayIconBuilder::with_id(ID_TRAY)
        .icon(Image::from_bytes(ICONA_GRIGIA)?)
        .tooltip(testo_tooltip(None, None, None))
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            "apri" => mostra_finestra(app),
            "check" => {
                mostra_finestra(app);
                let _ = app.emit("tray-azione", "check");
            }
            "sicurezza" => {
                mostra_finestra(app);
                let _ = app.emit("tray-azione", "sicurezza");
            }
            "pausa" => crate::monitor::alterna_pausa(app),
            "esci" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, evento| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = evento
            {
                mostra_finestra(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(VociTray { apri, check, sicurezza, pausa, esci });
    Ok(())
}

/// Aggiorna icona e tooltip.
pub fn aggiorna(app: &AppHandle, stato: Option<OverallStatus>, vpn: Option<bool>, ora: Option<&str>) {
    let Some(tray) = app.tray_by_id(ID_TRAY) else { return };
    if let Ok(img) = Image::from_bytes(icona_per(stato)) {
        let _ = tray.set_icon(Some(img));
    }
    let _ = tray.set_tooltip(Some(testo_tooltip(stato, vpn, ora)));
}

pub fn aggiorna_da_report(app: &AppHandle, r: &SecurityReport) {
    // Un controllo senza IP (errore di rete) non cambia il colore: resta l'ultimo verdetto valido
    if r.overall_status == OverallStatus::Error {
        return;
    }
    aggiorna(
        app,
        Some(r.overall_status),
        (!r.vpn_sconosciuta).then_some(r.is_vpn_or_proxy_ip),
        ora_da_iso(&r.timestamp).as_deref(),
    );
}

/// Dopo un cambio di lingua: testi del menu e tooltip (dall'ultimo controllo salvato).
pub fn aggiorna_lingua(app: &AppHandle, archivio: &crate::archivio::Archivio, monitoraggio_attivo: bool) {
    if let Some(voci) = app.try_state::<VociTray>() {
        let _ = voci.apri.set_text(sc("Apri", "Open"));
        let _ = voci.check.set_text(sc("Check completo", "Full check"));
        let _ = voci.sicurezza.set_text(sc("Solo sicurezza", "Security only"));
        let _ = voci.pausa.set_text(etichetta_pausa(monitoraggio_attivo));
        let _ = voci.esci.set_text(sc("Esci", "Quit"));
    }
    crate::tray_iniziale(app, archivio);
}

pub fn imposta_pausa(app: &AppHandle, monitoraggio_attivo: bool) {
    if let Some(voci) = app.try_state::<VociTray>() {
        let _ = voci.pausa.set_text(etichetta_pausa(monitoraggio_attivo));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_nei_vari_stati() {
        assert_eq!(
            testo_tooltip(Some(OverallStatus::Protected), Some(true), Some("10:54")),
            "Connessione — Protetta, VPN accesa, ultimo controllo 10:54"
        );
        assert_eq!(
            testo_tooltip(Some(OverallStatus::Unprotected), Some(false), Some("09:05")),
            "Connessione — Non protetta, VPN spenta, ultimo controllo 09:05"
        );
        assert_eq!(
            testo_tooltip(Some(OverallStatus::Warning), Some(true), Some("23:59")),
            "Connessione — Attenzione, VPN accesa, ultimo controllo 23:59"
        );
        assert_eq!(
            testo_tooltip(None, None, None),
            "Connessione — verdetto sconosciuto, VPN sconosciuta, nessun controllo"
        );
    }

    #[test]
    fn icone_per_verdetto_sono_png_distinti() {
        let tutte = [
            icona_per(Some(OverallStatus::Protected)),
            icona_per(Some(OverallStatus::Warning)),
            icona_per(Some(OverallStatus::Unprotected)),
            icona_per(None),
        ];
        for icona in tutte {
            assert_eq!(&icona[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
            assert!(Image::from_bytes(icona).is_ok());
        }
        for i in 0..tutte.len() {
            for j in i + 1..tutte.len() {
                assert_ne!(tutte[i], tutte[j]);
            }
        }
        assert_eq!(icona_per(Some(OverallStatus::Error)), icona_per(None));
    }

    #[test]
    fn ora_dal_timestamp() {
        assert_eq!(ora_da_iso("2026-11-23T10:54:01+01:00").as_deref(), Some("10:54"));
        assert_eq!(ora_da_iso("non una data"), None);
    }

    #[test]
    fn etichette_pausa() {
        assert_eq!(etichetta_pausa(true), "Pausa monitoraggio");
        assert_eq!(etichetta_pausa(false), "Riprendi monitoraggio");
    }
}

#[cfg(test)]
mod tests_lingua {
    use super::*;
    use crate::testi::{LinguaEffettiva, con_lingua};

    #[test]
    fn tooltip_e_menu_nelle_due_lingue() {
        let it = con_lingua(LinguaEffettiva::It, || testo_tooltip(Some(OverallStatus::Protected), Some(true), Some("10:54")));
        let en = con_lingua(LinguaEffettiva::En, || testo_tooltip(Some(OverallStatus::Protected), Some(true), Some("10:54")));
        assert_eq!(it, "Connessione — Protetta, VPN accesa, ultimo controllo 10:54");
        assert_eq!(en, "Connessione — Protected, VPN on, last check 10:54");
        let it = con_lingua(LinguaEffettiva::It, || testo_tooltip(None, None, None));
        let en = con_lingua(LinguaEffettiva::En, || testo_tooltip(None, None, None));
        assert!(!it.is_empty() && !en.is_empty());
        assert_ne!(it, en);
        for attivo in [true, false] {
            let a = con_lingua(LinguaEffettiva::It, || etichetta_pausa(attivo));
            let b = con_lingua(LinguaEffettiva::En, || etichetta_pausa(attivo));
            assert!(!a.is_empty() && !b.is_empty());
            assert_ne!(a, b);
        }
    }
}
