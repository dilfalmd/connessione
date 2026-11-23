//! Rilevamento della connessione a consumo (WinRT: costo della connessione a Internet).

use tauri::State;
use windows::Networking::Connectivity::{NetworkCostType, NetworkInformation};

use crate::impostazioni::ModalitaConsumo;
use crate::monitor::Monitor;

/// Decide se la connessione conta come a consumo: `windows_dice` è il giudizio di Windows.
pub fn a_consumo_effettivo(modalita: ModalitaConsumo, windows_dice: bool) -> bool {
    match modalita {
        ModalitaConsumo::SeguiWindows => windows_dice,
        ModalitaConsumo::Sempre => true,
        ModalitaConsumo::Ignora => false,
    }
}

/// Fixed e Variable valgono come "a consumo", come roaming e superamento del limite dati.
pub fn costo_a_consumo(tipo: NetworkCostType, roaming: bool, oltre_limite: bool) -> bool {
    tipo == NetworkCostType::Fixed || tipo == NetworkCostType::Variable || roaming || oltre_limite
}

fn lettura() -> windows::core::Result<bool> {
    let profilo = NetworkInformation::GetInternetConnectionProfile()?;
    let costo = profilo.GetConnectionCost()?;
    Ok(costo_a_consumo(
        costo.NetworkCostType()?,
        costo.Roaming()?,
        costo.OverDataLimit()?,
    ))
}

/// Vero se la connessione a Internet corrente è a consumo. Se il profilo non è leggibile
/// (nessuna connessione, errore WinRT) restituisce `false`.
pub fn connessione_a_consumo_sync() -> bool {
    lettura().unwrap_or(false)
}

/// Applica la modalità scelta; Windows si interroga solo se serve.
pub async fn a_consumo(modalita: ModalitaConsumo) -> bool {
    let windows_dice = match modalita {
        ModalitaConsumo::SeguiWindows => tokio::task::spawn_blocking(connessione_a_consumo_sync)
            .await
            .unwrap_or(false),
        ModalitaConsumo::Sempre | ModalitaConsumo::Ignora => false,
    };
    a_consumo_effettivo(modalita, windows_dice)
}

#[tauri::command]
pub async fn connessione_a_consumo(monitor: State<'_, Monitor>) -> Result<bool, String> {
    Ok(a_consumo(monitor.corrente().modalita_consumo).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regole_del_costo() {
        assert!(!costo_a_consumo(NetworkCostType::Unrestricted, false, false));
        assert!(!costo_a_consumo(NetworkCostType::Unknown, false, false));
        assert!(costo_a_consumo(NetworkCostType::Fixed, false, false));
        assert!(costo_a_consumo(NetworkCostType::Variable, false, false));
        assert!(costo_a_consumo(NetworkCostType::Unrestricted, true, false));
        assert!(costo_a_consumo(NetworkCostType::Unrestricted, false, true));
    }

    #[test]
    fn modalita_a_consumo_effettiva() {
        use ModalitaConsumo::*;
        assert!(!a_consumo_effettivo(SeguiWindows, false));
        assert!(a_consumo_effettivo(SeguiWindows, true));
        assert!(a_consumo_effettivo(Sempre, false));
        assert!(a_consumo_effettivo(Sempre, true));
        assert!(!a_consumo_effettivo(Ignora, false));
        assert!(!a_consumo_effettivo(Ignora, true));
    }

    #[tokio::test]
    async fn modalita_fisse_non_dipendono_da_windows() {
        assert!(a_consumo(ModalitaConsumo::Sempre).await);
        assert!(!a_consumo(ModalitaConsumo::Ignora).await);
    }

    #[test]
    fn lettura_di_sistema_non_va_in_panico() {
        // Il valore dipende dalla rete in uso: si verifica solo che la chiamata WinRT funzioni.
        let _ = connessione_a_consumo_sync();
    }
}
