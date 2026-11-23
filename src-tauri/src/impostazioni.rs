//! Impostazioni dell'app: struttura, valori predefiniti e validazione.
//! Salvate nella tabella `impostazioni` (chiave -> valore JSON), una riga per campo.

use serde::{Deserialize, Serialize};

use crate::testi::Lingua;

pub const INTERVALLI_SICUREZZA_MIN: [u32; 4] = [5, 15, 30, 60];
pub const INTERVALLI_SPEEDTEST_ORE: [u32; 5] = [1, 3, 6, 12, 24];

/// Come trattare le connessioni a consumo (speedtest manuale: conferma; automatico: saltato).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModalitaConsumo {
    /// Vale quanto dichiara Windows per la connessione corrente.
    #[default]
    SeguiWindows,
    /// Ogni connessione è trattata come a consumo.
    Sempre,
    /// Nessuna connessione è a consumo (giga illimitati), qualunque cosa dica Windows.
    Ignora,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Impostazioni {
    /// Controlli automatici attivi (anche a finestra nascosta).
    pub monitoraggio_attivo: bool,
    /// Ogni quanti minuti il controllo di sicurezza automatico (5, 15, 30, 60).
    pub intervallo_sicurezza_min: u32,
    /// Ogni quante ore lo speedtest automatico; `None` = spento (1, 3, 6, 12, 24).
    pub speedtest_ogni_ore: Option<u32>,
    /// Server scelto nella barra (`None` = automatico), usato dallo speedtest automatico.
    pub server_speedtest: Option<u32>,
    /// Ripetizioni scelte nella barra (1-5), usate dallo speedtest automatico.
    pub ripetizioni_speedtest: u8,
    /// Avviso se il download scende sotto questo valore (Mbps); `None` = spento.
    pub soglia_download_min_mbps: Option<f64>,
    /// Avviso se il ping supera questo valore (ms); `None` = spento.
    pub soglia_ping_max_ms: Option<f64>,
    /// Chiudendo la finestra l'app resta nella tray.
    pub chiudi_in_tray: bool,
    /// Avvio con Windows (con `--minimizzato`).
    pub avvio_con_windows: bool,
    pub notifiche_attive: bool,
    /// Come considerare le connessioni a consumo.
    pub modalita_consumo: ModalitaConsumo,
    /// L'utente ha accettato licenza, termini e informativa privacy di Ookla (richiesti dalla CLI).
    /// Si cambia solo con il comando `accetta_licenza_ookla`, mai con `salva_impostazioni`.
    pub licenza_ookla_accettata: bool,
    /// Lingua dell'interfaccia e dei testi: automatica (Windows), italiano, inglese.
    pub lingua: Lingua,
    /// Cerca una versione nuova su GitHub all'avvio (in silenzio se non c'è nulla). Il pulsante
    /// «Cerca aggiornamenti» delle impostazioni funziona comunque; non si installa mai senza un clic.
    pub controlla_aggiornamenti: bool,
}

impl Default for Impostazioni {
    fn default() -> Self {
        Impostazioni {
            monitoraggio_attivo: true,
            intervallo_sicurezza_min: 15,
            speedtest_ogni_ore: None,
            server_speedtest: None,
            ripetizioni_speedtest: 1,
            soglia_download_min_mbps: None,
            soglia_ping_max_ms: None,
            chiudi_in_tray: true,
            avvio_con_windows: false,
            notifiche_attive: true,
            modalita_consumo: ModalitaConsumo::SeguiWindows,
            licenza_ookla_accettata: false,
            lingua: Lingua::Auto,
            controlla_aggiornamenti: true,
        }
    }
}

fn soglia_valida(v: Option<f64>) -> Option<f64> {
    v.filter(|x| x.is_finite() && *x > 0.0)
}

impl Impostazioni {
    /// Riporta ogni valore a uno ammesso (valori non previsti tornano al predefinito).
    pub fn validata(mut self) -> Self {
        let pred = Impostazioni::default();
        if !INTERVALLI_SICUREZZA_MIN.contains(&self.intervallo_sicurezza_min) {
            self.intervallo_sicurezza_min = pred.intervallo_sicurezza_min;
        }
        if let Some(h) = self.speedtest_ogni_ore
            && !INTERVALLI_SPEEDTEST_ORE.contains(&h)
        {
            self.speedtest_ogni_ore = None;
        }
        self.ripetizioni_speedtest = self.ripetizioni_speedtest.clamp(1, 5);
        self.soglia_download_min_mbps = soglia_valida(self.soglia_download_min_mbps);
        self.soglia_ping_max_ms = soglia_valida(self.soglia_ping_max_ms);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predefiniti_come_da_decisione() {
        let i = Impostazioni::default();
        assert!(i.monitoraggio_attivo);
        assert_eq!(i.intervallo_sicurezza_min, 15);
        assert_eq!(i.speedtest_ogni_ore, None);
        assert_eq!(i.soglia_download_min_mbps, None);
        assert_eq!(i.soglia_ping_max_ms, None);
        assert!(i.chiudi_in_tray);
        assert!(!i.avvio_con_windows);
        assert!(i.notifiche_attive);
        assert_eq!(i.modalita_consumo, ModalitaConsumo::SeguiWindows);
        assert!(!i.licenza_ookla_accettata);
        assert_eq!(i.lingua, Lingua::Auto);
        assert!(i.controlla_aggiornamenti, "il controllo all'avvio è attivo di serie");
    }

    #[test]
    fn validazione_riporta_ai_valori_ammessi() {
        let i = Impostazioni {
            intervallo_sicurezza_min: 7,
            speedtest_ogni_ore: Some(5),
            ripetizioni_speedtest: 9,
            soglia_download_min_mbps: Some(-3.0),
            soglia_ping_max_ms: Some(f64::NAN),
            ..Default::default()
        }
        .validata();
        assert_eq!(i.intervallo_sicurezza_min, 15);
        assert_eq!(i.speedtest_ogni_ore, None);
        assert_eq!(i.ripetizioni_speedtest, 5);
        assert_eq!(i.soglia_download_min_mbps, None);
        assert_eq!(i.soglia_ping_max_ms, None);
        let ok = Impostazioni {
            intervallo_sicurezza_min: 60,
            speedtest_ogni_ore: Some(24),
            soglia_download_min_mbps: Some(50.0),
            soglia_ping_max_ms: Some(80.0),
            ..Default::default()
        }
        .validata();
        assert_eq!(ok.intervallo_sicurezza_min, 60);
        assert_eq!(ok.speedtest_ogni_ore, Some(24));
        assert_eq!(ok.soglia_download_min_mbps, Some(50.0));
    }

    #[test]
    fn campi_mancanti_nel_json_usano_i_predefiniti() {
        let i: Impostazioni = serde_json::from_value(serde_json::json!({"intervalloSicurezzaMin": 30})).unwrap();
        assert_eq!(i.intervallo_sicurezza_min, 30);
        assert!(i.notifiche_attive);
    }

    #[test]
    fn impostazioni_salvate_prima_del_lotto_6_usano_i_predefiniti_nuovi() {
        // JSON di una versione precedente: nessuna modalità a consumo, nessuna licenza
        let i: Impostazioni = serde_json::from_value(serde_json::json!({
            "monitoraggioAttivo": false,
            "notificheAttive": false
        }))
        .unwrap();
        assert_eq!(i.modalita_consumo, ModalitaConsumo::SeguiWindows);
        assert!(!i.licenza_ookla_accettata);
        assert_eq!(i.lingua, Lingua::Auto, "le impostazioni vecchie non hanno la lingua: Auto");
        assert!(i.controlla_aggiornamenti, "le impostazioni vecchie non hanno il campo: si controlla");
    }

    #[test]
    fn controllo_aggiornamenti_spento_si_conserva() {
        let i: Impostazioni = serde_json::from_value(serde_json::json!({"controllaAggiornamenti": false})).unwrap();
        assert!(!i.controlla_aggiornamenti);
        let j = serde_json::to_value(&i).unwrap();
        assert_eq!(j["controllaAggiornamenti"], false);
    }

    /// Salvaguardia di pubblicazione: le tre versioni coincidono e l'updater ha chiave pubblica
    /// e indirizzo di GitHub (senza, la release non si aggiornerebbe da sola).
    #[test]
    fn versioni_e_configurazione_updater_coerenti() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let pkg: serde_json::Value = serde_json::from_str(include_str!("../../package.json")).unwrap();
        let v = env!("CARGO_PKG_VERSION");
        assert_eq!(conf["version"], v, "tauri.conf.json");
        assert_eq!(pkg["version"], v, "package.json");
        assert_eq!(conf["bundle"]["createUpdaterArtifacts"], true);
        let up = &conf["plugins"]["updater"];
        assert!(up["pubkey"].as_str().is_some_and(|k| k.len() > 50), "chiave pubblica mancante");
        assert_eq!(
            up["endpoints"][0],
            "https://github.com/dilfalmd/connessione/releases/latest/download/latest.json"
        );
        assert_eq!(up["windows"]["installMode"], "passive");
    }

    #[test]
    fn modalita_consumo_serializzata_in_camel_case() {
        let j = serde_json::to_value(Impostazioni::default()).unwrap();
        assert_eq!(j["modalitaConsumo"], "seguiWindows");
        for (m, t) in [(ModalitaConsumo::Sempre, "sempre"), (ModalitaConsumo::Ignora, "ignora")] {
            assert_eq!(serde_json::to_value(m).unwrap(), t);
            assert_eq!(serde_json::from_value::<ModalitaConsumo>(serde_json::json!(t)).unwrap(), m);
        }
    }
}
