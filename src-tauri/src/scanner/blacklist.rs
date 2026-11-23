//! Controllo DNSBL dell'IPv4 pubblico su liste internazionali, tramite il resolver di sistema.
//!
//! Ogni lista restituisce uno stato a tre valori (in lista / pulito / non verificabile) in base
//! all'indirizzo 127.x.x.x restituito, non al semplice "risolve / non risolve".

use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::net::lookup_host;
use tokio::task::JoinSet;
use tokio::time::timeout;
use crate::testi::{sc, tr};

pub const DNSBL_PROVIDERS: [(&str, &str); 6] = [
    ("Spamhaus ZEN", "zen.spamhaus.org"),
    ("SpamCop", "bl.spamcop.net"),
    ("Barracuda", "b.barracudacentral.org"),
    ("SORBS DNSBL", "dnsbl.sorbs.net"),
    ("UCEPROTECT L1", "dnsbl-1.uceprotect.net"),
    ("Backscatterer", "ips.backscatterer.org"),
];

const ZONA_SPAMHAUS: &str = "zen.spamhaus.org";

/// Limite di sicurezza per una singola risoluzione (il Python non ne ha uno proprio).
const TIMEOUT_QUERY: Duration = Duration::from_secs(8);

/// Codici WSA di Windows per "nome inesistente" (HOST_NOT_FOUND) e "nessun dato" (NO_DATA).
const WSAHOST_NOT_FOUND: i32 = 11001;
const WSANO_DATA: i32 = 11004;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatoDnsbl {
    InLista,
    Pulito,
    NonVerificabile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoceDnsbl {
    pub stato: StatoDnsbl,
    pub nota: Option<String>,
}

impl VoceDnsbl {
    pub fn nuova(stato: StatoDnsbl, nota: Option<String>) -> Self {
        VoceDnsbl { stato, nota }
    }
}

/// Esito grezzo di una interrogazione DNS alla zona.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EsitoQuery {
    /// NXDOMAIN / nessun record: l'IP non e` nella lista.
    NonEsiste,
    /// Indirizzi IPv4 restituiti.
    Risposta(Vec<Ipv4Addr>),
    /// Timeout o errore di rete/resolver.
    Errore(String),
}

/// Nome da interrogare: ottetti dell'IP in ordine inverso + zona.
pub fn nome_query(ip: &str, zona: &str) -> String {
    let inverso: Vec<&str> = ip.split('.').rev().collect();
    format!("{}.{}", inverso.join("."), zona)
}

fn e_errore_interrogazione(a: &Ipv4Addr) -> bool {
    let o = a.octets();
    o[0] == 127 && o[1] == 255 && o[2] == 255
}

/// 127.0.0.10 e .11 di Spamhaus ZEN: Policy Block List (IP residenziali/dinamici).
fn e_pbl_spamhaus(zona: &str, a: &Ipv4Addr) -> bool {
    let o = a.octets();
    zona == ZONA_SPAMHAUS && o[0] == 127 && o[1] == 0 && o[2] == 0 && (o[3] == 10 || o[3] == 11)
}

/// Interpreta l'esito di una interrogazione per una zona.
///
/// - 127.0.0.0/8 (esclusi 127.255.255.x): in lista; per Spamhaus ZEN, 127.0.0.10-11 (PBL) sono
///   "pulito" con una nota, 127.0.0.2-9 (SBL/CSS/XBL) sono in lista.
/// - 127.255.255.x: errore di interrogazione (resolver pubblico o limite): non verificabile.
/// - qualsiasi altro indirizzo, timeout o errore: non verificabile.
pub fn classifica(zona: &str, esito: &EsitoQuery) -> VoceDnsbl {
    match esito {
        EsitoQuery::NonEsiste => VoceDnsbl::nuova(StatoDnsbl::Pulito, None),
        EsitoQuery::Errore(motivo) => VoceDnsbl::nuova(
            StatoDnsbl::NonVerificabile,
            Some(tr!("Interrogazione fallita: {motivo}", "Query failed: {motivo}")),
        ),
        EsitoQuery::Risposta(indirizzi) => {
            let in_lista = indirizzi.iter().find(|a| {
                a.octets()[0] == 127 && !e_errore_interrogazione(a) && !e_pbl_spamhaus(zona, a)
            });
            if let Some(a) = in_lista {
                return VoceDnsbl::nuova(StatoDnsbl::InLista, Some(tr!("Risposta {a}", "Response {a}")));
            }
            if let Some(a) = indirizzi.iter().find(|a| e_pbl_spamhaus(zona, a)) {
                return VoceDnsbl::nuova(
                    StatoDnsbl::Pulito,
                    Some(tr!(
                        "IP residenziale/dinamico (PBL, {a}): non è una blacklist di abusi",
                        "Residential/dynamic IP (PBL, {a}): not an abuse blacklist"
                    )),
                );
            }
            let nota = match indirizzi.first() {
                Some(a) if e_errore_interrogazione(a) => tr!(
                    "Risposta {a}: interrogazione bloccata (resolver pubblico o limite di richieste)",
                    "Response {a}: query blocked (public resolver or rate limit)"
                ),
                Some(a) => {
                    tr!("Risposta non valida {a}: il resolver non risponde come una DNSBL", "Invalid response {a}: the resolver does not answer like a DNSBL")
                }
                None => sc("Nessun indirizzo IPv4 nella risposta", "No IPv4 address in the response").to_string(),
            };
            VoceDnsbl::nuova(StatoDnsbl::NonVerificabile, Some(nota))
        }
    }
}

async fn interroga(ip: &str, zona: &str) -> EsitoQuery {
    let query = nome_query(ip, zona);
    match timeout(TIMEOUT_QUERY, lookup_host((query.as_str(), 0))).await {
        Err(_) => EsitoQuery::Errore("timeout".into()),
        Ok(Err(e)) => match e.raw_os_error() {
            Some(WSAHOST_NOT_FOUND) | Some(WSANO_DATA) => EsitoQuery::NonEsiste,
            _ => EsitoQuery::Errore(e.to_string()),
        },
        Ok(Ok(indirizzi)) => EsitoQuery::Risposta(
            indirizzi
                .filter_map(|a| match a.ip() {
                    IpAddr::V4(v4) => Some(v4),
                    IpAddr::V6(_) => None,
                })
                .collect(),
        ),
    }
}

/// Controlla l'IPv4 su tutte le liste in parallelo; risultati nell'ordine dei provider.
pub async fn controlla_tutte(ipv4: Option<&str>) -> IndexMap<String, VoceDnsbl> {
    let mut risultati: IndexMap<String, VoceDnsbl> = DNSBL_PROVIDERS
        .iter()
        .map(|(nome, _)| {
            (
                nome.to_string(),
                VoceDnsbl::nuova(
                    StatoDnsbl::NonVerificabile,
                    Some(sc("Controllo non eseguito o non completato", "Check not run or not completed").into()),
                ),
            )
        })
        .collect();
    let Some(ip) = ipv4.filter(|s| s.contains('.')) else {
        return risultati;
    };
    let mut set = JoinSet::new();
    for (idx, (_, zona)) in DNSBL_PROVIDERS.iter().enumerate() {
        let ip = ip.to_string();
        set.spawn(async move {
            let esito = interroga(&ip, zona).await;
            (idx, classifica(zona, &esito))
        });
    }
    while let Some(res) = set.join_next().await {
        if let Ok((idx, voce)) = res {
            risultati[DNSBL_PROVIDERS[idx].0] = voce;
        }
    }
    risultati
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risposta(zona: &str, ip: [u8; 4]) -> VoceDnsbl {
        classifica(zona, &EsitoQuery::Risposta(vec![Ipv4Addr::from(ip)]))
    }

    #[test]
    fn query_con_ip_invertito() {
        assert_eq!(
            nome_query("203.0.113.7", "zen.spamhaus.org"),
            "7.113.0.203.zen.spamhaus.org"
        );
        assert_eq!(nome_query("1.2.3.4", "bl.spamcop.net"), "4.3.2.1.bl.spamcop.net");
    }

    #[test]
    fn sei_provider_come_nel_python() {
        assert_eq!(DNSBL_PROVIDERS.len(), 6);
        assert_eq!(DNSBL_PROVIDERS[0], ("Spamhaus ZEN", "zen.spamhaus.org"));
        assert_eq!(DNSBL_PROVIDERS[5], ("Backscatterer", "ips.backscatterer.org"));
    }

    #[test]
    fn nxdomain_e_pulito() {
        let v = classifica(ZONA_SPAMHAUS, &EsitoQuery::NonEsiste);
        assert_eq!(v.stato, StatoDnsbl::Pulito);
        assert!(v.nota.is_none());
    }

    #[test]
    fn spamhaus_sbl_css_xbl_127_0_0_2_a_9_in_lista() {
        for ultimo in 2..=9u8 {
            let v = risposta(ZONA_SPAMHAUS, [127, 0, 0, ultimo]);
            assert_eq!(v.stato, StatoDnsbl::InLista, "127.0.0.{ultimo}");
        }
        assert!(risposta(ZONA_SPAMHAUS, [127, 0, 0, 2]).nota.unwrap().contains("127.0.0.2"));
    }

    #[test]
    fn spamhaus_pbl_127_0_0_10_e_11_non_in_lista() {
        for ultimo in [10u8, 11] {
            let v = risposta(ZONA_SPAMHAUS, [127, 0, 0, ultimo]);
            assert_eq!(v.stato, StatoDnsbl::Pulito, "127.0.0.{ultimo}");
            assert!(v.nota.unwrap().contains("PBL"));
        }
    }

    #[test]
    fn spamhaus_pbl_insieme_a_sbl_resta_in_lista() {
        let v = classifica(
            ZONA_SPAMHAUS,
            &EsitoQuery::Risposta(vec![Ipv4Addr::new(127, 0, 0, 11), Ipv4Addr::new(127, 0, 0, 4)]),
        );
        assert_eq!(v.stato, StatoDnsbl::InLista);
    }

    #[test]
    fn codici_10_e_11_su_altre_zone_sono_in_lista() {
        assert_eq!(risposta("bl.spamcop.net", [127, 0, 0, 10]).stato, StatoDnsbl::InLista);
    }

    #[test]
    fn errore_di_interrogazione_127_255_255_x() {
        for ultimo in [252u8, 254, 255] {
            let v = risposta(ZONA_SPAMHAUS, [127, 255, 255, ultimo]);
            assert_eq!(v.stato, StatoDnsbl::NonVerificabile, "127.255.255.{ultimo}");
            assert!(v.nota.unwrap().contains("interrogazione bloccata"));
        }
        assert_eq!(
            risposta("dnsbl.sorbs.net", [127, 255, 255, 254]).stato,
            StatoDnsbl::NonVerificabile
        );
    }

    #[test]
    fn altre_zone_127_0_0_x_in_lista() {
        for ip in [[127, 0, 0, 2], [127, 0, 0, 1], [127, 0, 0, 255], [127, 1, 2, 3]] {
            assert_eq!(risposta("b.barracudacentral.org", ip).stato, StatoDnsbl::InLista, "{ip:?}");
        }
    }

    #[test]
    fn indirizzo_fuori_da_127_non_verificabile() {
        let v = risposta("dnsbl-1.uceprotect.net", [93, 184, 216, 34]);
        assert_eq!(v.stato, StatoDnsbl::NonVerificabile);
        assert!(v.nota.unwrap().contains("93.184.216.34"));
    }

    #[test]
    fn errori_e_risposte_vuote_non_verificabili() {
        let e = classifica("bl.spamcop.net", &EsitoQuery::Errore("timeout".into()));
        assert_eq!(e.stato, StatoDnsbl::NonVerificabile);
        let v = classifica("bl.spamcop.net", &EsitoQuery::Risposta(vec![]));
        assert_eq!(v.stato, StatoDnsbl::NonVerificabile);
    }

    #[tokio::test]
    async fn senza_ipv4_nessuna_lista_risulta_pulita() {
        let r = controlla_tutte(None).await;
        assert_eq!(r.len(), DNSBL_PROVIDERS.len());
        assert!(r.values().all(|v| v.stato == StatoDnsbl::NonVerificabile));
    }

    #[test]
    fn serializzazione_snake_case() {
        let v = VoceDnsbl::nuova(StatoDnsbl::NonVerificabile, Some("x".into()));
        let j = serde_json::to_value(&v).unwrap();
        assert_eq!(j["stato"], "non_verificabile");
        assert_eq!(j["nota"], "x");
        assert_eq!(serde_json::to_value(StatoDnsbl::InLista).unwrap(), "in_lista");
    }
}
