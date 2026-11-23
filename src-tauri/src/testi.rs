//! Lingua dell'app per i testi prodotti dal Rust (verdetti dello scanner, errori, tray,
//! notifiche, export .txt). La lingua corrente è un valore globale: si imposta all'avvio e quando
//! cambia l'impostazione `lingua`; ogni testo nasce già nella lingua corrente (niente codici).
//! I controlli già salvati nello storico restano nella lingua in cui sono stati scritti.

use std::sync::atomic::{AtomicU8, Ordering};

use serde::{Deserialize, Serialize};

/// Scelta dell'utente nelle impostazioni.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lingua {
    /// Segue la lingua di Windows: italiano se è italiano, altrimenti inglese.
    #[default]
    Auto,
    It,
    En,
}

/// Lingua realmente in uso.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LinguaEffettiva {
    It,
    En,
}

impl LinguaEffettiva {
    pub fn codice(self) -> &'static str {
        match self {
            LinguaEffettiva::It => "it",
            LinguaEffettiva::En => "en",
        }
    }
}

static CORRENTE: AtomicU8 = AtomicU8::new(0); // 0 = italiano, 1 = inglese

#[cfg(test)]
thread_local! {
    /// Solo nei test: lingua forzata per il thread corrente (i test paralleli non si disturbano).
    static FORZATA: std::cell::Cell<Option<LinguaEffettiva>> = const { std::cell::Cell::new(None) };
}

/// Lingua corrente dei testi.
pub fn corrente() -> LinguaEffettiva {
    #[cfg(test)]
    if let Some(l) = FORZATA.with(|f| f.get()) {
        return l;
    }
    if CORRENTE.load(Ordering::Relaxed) == 1 { LinguaEffettiva::En } else { LinguaEffettiva::It }
}

pub fn inglese() -> bool {
    corrente() == LinguaEffettiva::En
}

pub fn imposta(lingua: LinguaEffettiva) {
    CORRENTE.store(u8::from(lingua == LinguaEffettiva::En), Ordering::Relaxed);
}

/// «it», «it-IT», «it_IT» -> italiano; qualunque altro locale -> inglese.
pub fn lingua_da_locale(locale: &str) -> LinguaEffettiva {
    let l = locale.trim().to_ascii_lowercase();
    let primo = l.split(['-', '_']).next().unwrap_or("");
    if primo == "it" { LinguaEffettiva::It } else { LinguaEffettiva::En }
}

/// Risolve la scelta dell'utente. `locale_windows` è la lingua di Windows, se leggibile
/// (se non lo è, `Auto` resta italiano).
pub fn risolvi(scelta: Lingua, locale_windows: Option<&str>) -> LinguaEffettiva {
    match scelta {
        Lingua::It => LinguaEffettiva::It,
        Lingua::En => LinguaEffettiva::En,
        Lingua::Auto => locale_windows.map(lingua_da_locale).unwrap_or(LinguaEffettiva::It),
    }
}

/// Risolve la scelta con la lingua di Windows e la rende corrente.
pub fn applica(scelta: Lingua) -> LinguaEffettiva {
    let l = risolvi(scelta, sys_locale::get_locale().as_deref());
    imposta(l);
    l
}

/// Sceglie fra due testi fissi.
pub fn sc(it: &'static str, en: &'static str) -> &'static str {
    if inglese() { en } else { it }
}

/// «Sconosciuto» / «Unknown».
pub fn sconosciuto() -> &'static str {
    sc("Sconosciuto", "Unknown")
}

/// `tr!("testo {x}", "text {x}", x = ...)`: come `format!`, ma con la versione italiana o inglese
/// secondo la lingua corrente. Restituisce una `String`.
macro_rules! tr {
    ($it:literal, $en:literal $(, $($arg:tt)*)?) => {
        if $crate::testi::inglese() { format!($en $(, $($arg)*)?) } else { format!($it $(, $($arg)*)?) }
    };
}
pub(crate) use tr;

/// Esegue `f` con la lingua forzata (solo test).
#[cfg(test)]
pub fn con_lingua<T>(lingua: LinguaEffettiva, f: impl FnOnce() -> T) -> T {
    let prima = FORZATA.with(|c| c.replace(Some(lingua)));
    let r = f();
    FORZATA.with(|c| c.set(prima));
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_di_windows_in_lingua() {
        assert_eq!(lingua_da_locale("it-IT"), LinguaEffettiva::It);
        assert_eq!(lingua_da_locale("it"), LinguaEffettiva::It);
        assert_eq!(lingua_da_locale("IT_ch"), LinguaEffettiva::It);
        assert_eq!(lingua_da_locale("en-US"), LinguaEffettiva::En);
        assert_eq!(lingua_da_locale("de-DE"), LinguaEffettiva::En);
        assert_eq!(lingua_da_locale("ita-IT"), LinguaEffettiva::En, "solo il codice «it» è italiano");
    }

    #[test]
    fn auto_si_risolve_dal_locale() {
        assert_eq!(risolvi(Lingua::Auto, Some("it-IT")), LinguaEffettiva::It);
        assert_eq!(risolvi(Lingua::Auto, Some("en-US")), LinguaEffettiva::En);
        assert_eq!(risolvi(Lingua::Auto, Some("de-DE")), LinguaEffettiva::En);
        assert_eq!(risolvi(Lingua::Auto, None), LinguaEffettiva::It);
        assert_eq!(risolvi(Lingua::It, Some("en-US")), LinguaEffettiva::It);
        assert_eq!(risolvi(Lingua::En, Some("it-IT")), LinguaEffettiva::En);
    }

    #[test]
    fn scelta_serializzata_in_minuscolo() {
        assert_eq!(serde_json::to_value(Lingua::Auto).unwrap(), "auto");
        assert_eq!(serde_json::from_value::<Lingua>(serde_json::json!("en")).unwrap(), Lingua::En);
        assert_eq!(serde_json::to_value(LinguaEffettiva::It).unwrap(), "it");
    }

    #[test]
    fn macro_e_scelta_seguono_la_lingua() {
        let x = 3;
        assert_eq!(tr!("{x} cose", "{x} things"), "3 cose");
        assert_eq!(sc("sì", "yes"), "sì");
        con_lingua(LinguaEffettiva::En, || {
            assert_eq!(tr!("{x} cose", "{x} things"), "3 things");
            assert_eq!(tr!("{} cose", "{} things", x + 1), "4 things");
            assert_eq!(sc("sì", "yes"), "yes");
        });
        assert_eq!(sc("sì", "yes"), "sì", "la lingua forzata finisce con la chiusura");
    }
}
