//! Tipi condivisi con il frontend (serde camelCase).

use std::fmt;

use serde::{Deserialize, Serialize};
use crate::testi::{sc, tr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FaseSpeedtest {
    Ping,
    Download,
    Upload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerSpeedtest {
    pub id: u32,
    pub nome: String,
    pub luogo: String,
    pub paese: String,
    pub host: String,
}

/// Payload dell'evento Tauri `speedtest-progresso`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressoSpeedtest {
    /// Ripetizione in corso, da 1.
    pub ripetizione: u8,
    pub totale: u8,
    pub fase: FaseSpeedtest,
    /// 0 - 1 all'interno della fase.
    pub avanzamento: f64,
    /// Valore istantaneo: ms per il ping, Mbps per download e upload.
    pub valore: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSpeedtest {
    pub ping_ms: f64,
    pub jitter_ms: Option<f64>,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    /// Media delle sole misure che lo riportano; `None` se nessuna lo ha.
    pub packet_loss: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RisultatoSpeedtest {
    pub sessione: String,
    pub misure: Vec<crate::archivio::MisuraVelocita>,
    pub media: MediaSpeedtest,
}

/// Errore tipizzato dei comandi speedtest: `{ "tipo": "CliMancante" }`,
/// `{ "tipo": "Fallito", "dettaglio": "..." }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", content = "dettaglio")]
pub enum ErroreSpeedtest {
    /// speedtest.exe (Ookla CLI) non trovato.
    CliMancante,
    Annullato,
    GiaInEsecuzione,
    /// L'utente non ha ancora accettato la licenza di Ookla.
    LicenzaNonAccettata,
    ArgomentoNonValido(String),
    Fallito(String),
}

impl fmt::Display for ErroreSpeedtest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErroreSpeedtest::CliMancante => {
                f.write_str(sc("Speedtest CLI di Ookla non trovata", "Ookla Speedtest CLI not found"))
            }
            ErroreSpeedtest::Annullato => f.write_str(sc("Test annullato", "Test cancelled")),
            ErroreSpeedtest::GiaInEsecuzione => {
                f.write_str(sc("Un test è già in esecuzione", "A test is already running"))
            }
            ErroreSpeedtest::LicenzaNonAccettata => f.write_str(sc(
                "Licenza di Ookla non accettata: accetta licenza, termini e informativa privacy per usare lo speedtest",
                "Ookla license not accepted: accept the license, terms and privacy policy to use the speed test",
            )),
            ErroreSpeedtest::ArgomentoNonValido(m) => f.write_str(&tr!("Argomento non valido: {m}", "Invalid argument: {m}")),
            ErroreSpeedtest::Fallito(m) => f.write_str(&tr!("Speedtest fallito: {m}", "Speed test failed: {m}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errore_serializzato_con_tipo() {
        let j = serde_json::to_value(ErroreSpeedtest::CliMancante).unwrap();
        assert_eq!(j, serde_json::json!({"tipo": "CliMancante"}));
        let j = serde_json::to_value(ErroreSpeedtest::Fallito("x".into())).unwrap();
        assert_eq!(j, serde_json::json!({"tipo": "Fallito", "dettaglio": "x"}));
        let j = serde_json::to_value(ErroreSpeedtest::LicenzaNonAccettata).unwrap();
        assert_eq!(j, serde_json::json!({"tipo": "LicenzaNonAccettata"}));
    }

    #[test]
    fn fase_in_minuscolo() {
        assert_eq!(serde_json::to_value(FaseSpeedtest::Download).unwrap(), "download");
    }
}

#[cfg(test)]
mod tests_lingua {
    use super::*;
    use crate::testi::{LinguaEffettiva, con_lingua};

    #[test]
    fn errori_nelle_due_lingue() {
        let errori = [
            ErroreSpeedtest::CliMancante,
            ErroreSpeedtest::Annullato,
            ErroreSpeedtest::GiaInEsecuzione,
            ErroreSpeedtest::LicenzaNonAccettata,
            ErroreSpeedtest::ArgomentoNonValido("x".into()),
            ErroreSpeedtest::Fallito("y".into()),
        ];
        for e in errori {
            let it = con_lingua(LinguaEffettiva::It, || e.to_string());
            let en = con_lingua(LinguaEffettiva::En, || e.to_string());
            assert!(!it.is_empty() && !en.is_empty());
            assert_ne!(it, en, "{e:?}");
        }
        let en = con_lingua(LinguaEffettiva::En, || ErroreSpeedtest::CliMancante.to_string());
        assert_eq!(en, "Ookla Speedtest CLI not found");
    }
}
