//! Check completo: controllo di sicurezza + speedtest collegato al controllo.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::archivio::Archivio;
use crate::monitor::Monitor;
use crate::scanner::{self, SecurityReport};
use crate::speedtest::{self, ErroreSpeedtest, RisultatoSpeedtest, StatoSpeedtest};

pub const EVENTO_FASE: &str = "check-fase";

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FaseCheck {
    Sicurezza,
    Velocita,
    Fatto,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventoFase {
    pub fase: FaseCheck,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RisultatoCheck {
    pub controllo: SecurityReport,
    /// `None` se il salvataggio nell'archivio non è riuscito.
    pub controllo_id: Option<i64>,
    pub velocita: Option<RisultatoSpeedtest>,
    /// Perché la parte di velocità manca (CLI mancante, annullato, errore).
    pub errore_velocita: Option<ErroreSpeedtest>,
}

pub(crate) fn emetti(app: &AppHandle, fase: FaseCheck) {
    let _ = app.emit(EVENTO_FASE, EventoFase { fase });
}

/// Esegue la scansione di sicurezza, la salva e lancia lo speedtest collegato al controllo.
/// Eventi: `check-fase` {fase} e, durante lo speedtest, `speedtest-progresso`.
/// Si annulla con `annulla_speedtest`.
#[tauri::command]
pub async fn check_completo(
    app: AppHandle,
    stato: State<'_, StatoSpeedtest>,
    archivio: State<'_, Archivio>,
    monitor: State<'_, Monitor>,
    server_id: Option<u32>,
    ripetizioni: u8,
) -> Result<RisultatoCheck, String> {
    speedtest::controlla_ripetizioni(ripetizioni).map_err(|e| e.to_string())?;
    let _permesso = stato.acquisisci().map_err(|e| e.to_string())?;

    emetti(&app, FaseCheck::Sicurezza);
    let report = match scanner::esegui_scansione().await {
        Ok(r) => r,
        Err(e) => {
            emetti(&app, FaseCheck::Fatto);
            return Err(e);
        }
    };
    let controllo_id = archivio.salva_se_valido(&report);

    crate::monitor::dopo_controllo(&app, &report, controllo_id, false);

    let mut velocita = None;
    let mut errore_velocita = None;
    let licenza = monitor.corrente().licenza_ookla_accettata;
    if stato.e_annullato() {
        errore_velocita = Some(ErroreSpeedtest::Annullato);
    } else if let Err(e) = speedtest::controlla_licenza(licenza) {
        // Senza licenza accettata il controllo di sicurezza resta valido, lo speedtest no
        errore_velocita = Some(e);
    } else {
        match speedtest::trova_cli() {
            Err(e) => errore_velocita = Some(e),
            Ok(exe) => {
                emetti(&app, FaseCheck::Velocita);
                let parametri = speedtest::ParametriSerie {
                    server_id,
                    ripetizioni,
                    controllo_id,
                    licenza_accettata: licenza,
                };
                let esito = speedtest::serie(&app, &stato, &archivio, &exe, parametri).await;
                // Anche le misure parziali passano da notifiche e soglie
                crate::monitor::dopo_misure(&app, &esito.misure, false);
                match esito.in_risultato() {
                    Ok(r) => velocita = Some(r),
                    Err(e) => errore_velocita = Some(e),
                }
            }
        }
    }
    emetti(&app, FaseCheck::Fatto);
    Ok(RisultatoCheck {
        controllo: report,
        controllo_id,
        velocita,
        errore_velocita,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eventi_e_risultato_serializzati() {
        let j = serde_json::to_value(EventoFase { fase: FaseCheck::Sicurezza }).unwrap();
        assert_eq!(j, serde_json::json!({"fase": "sicurezza"}));
        assert_eq!(serde_json::to_value(FaseCheck::Velocita).unwrap(), "velocita");
        assert_eq!(serde_json::to_value(FaseCheck::Fatto).unwrap(), "fatto");
        let r = RisultatoCheck {
            controllo: SecurityReport::vuoto("t".into()),
            controllo_id: Some(3),
            velocita: None,
            errore_velocita: Some(ErroreSpeedtest::CliMancante),
        };
        let j = serde_json::to_value(&r).unwrap();
        assert_eq!(j["controlloId"], 3);
        assert_eq!(j["erroreVelocita"]["tipo"], "CliMancante");
        assert!(j["velocita"].is_null());
    }
}
