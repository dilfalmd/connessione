//! DNS leak test: query HTTP verso sottodomini casuali di edns.ip-api.com, che rispondono con
//! il resolver DNS che ha interrogato il loro server autoritativo (ed eventuale EDNS Client Subnet).

use std::time::Duration;

use indexmap::IndexMap;
use reqwest::Client;
use serde_json::Value;
use tokio::task::JoinSet;

use super::ip::get_json;
use super::report::DnsServerEntry;
use crate::testi::sconosciuto;

pub const NUM_QUERIES: usize = 4;
const TIMEOUT_QUERY: Duration = Duration::from_millis(3500);
const ALFABETO: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// Prefisso casuale di 32 caratteri [a-z0-9].
pub fn prefisso_casuale() -> String {
    let mut buf = [0u8; 32];
    if getrandom::fill(&mut buf).is_err() {
        // Fallback molto improbabile: derivato dal clock, comunque diverso a ogni chiamata.
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (i, b) in buf.iter_mut().enumerate() {
            *b = ((n >> (i % 16)) as u8).wrapping_add(i as u8);
        }
    }
    buf.iter()
        .map(|b| ALFABETO[*b as usize % ALFABETO.len()] as char)
        .collect()
}

/// Interpreta la risposta di edns.ip-api.com. `None` se manca l'IP del resolver.
pub fn parse_edns(data: &Value) -> Option<DnsServerEntry> {
    let dns_info = data.get("dns")?.as_object()?;
    let dns_ip = dns_info.get("ip").and_then(Value::as_str).filter(|s| !s.is_empty())?;
    let dns_geo = dns_info
        .get("geo")
        .and_then(Value::as_str)
        .unwrap_or(sconosciuto());
    let edns = data.get("edns").and_then(Value::as_object);
    let edns_ip = edns
        .and_then(|e| e.get("ip"))
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let edns_geo = edns
        .and_then(|e| e.get("geo"))
        .and_then(Value::as_str)
        .map(str::to_string);
    Some(DnsServerEntry {
        ip: dns_ip.to_string(),
        geo_org: dns_geo.to_string(),
        edns_ip,
        edns_geo_org: edns_geo,
        is_leak_suspect: false,
    })
}

/// Aggiunge un resolver trovato: se gia` noto ma senza dati ECS, completa quelli.
pub fn unisci_resolver(trovati: &mut IndexMap<String, DnsServerEntry>, entry: DnsServerEntry) {
    match trovati.get_mut(&entry.ip) {
        None => {
            trovati.insert(entry.ip.clone(), entry);
        }
        Some(esistente) => {
            if entry.edns_ip.is_some() && esistente.edns_ip.is_none() {
                esistente.edns_ip = entry.edns_ip;
                esistente.edns_geo_org = entry.edns_geo_org;
            }
        }
    }
}

async fn query_singola(client: Client) -> Option<DnsServerEntry> {
    let url = format!("http://{}.edns.ip-api.com/json", prefisso_casuale());
    let data = get_json(&client, &url, TIMEOUT_QUERY).await?;
    parse_edns(&data)
}

/// Esegue `n` query in parallelo per scoprire i resolver DNS attivi (ordine di arrivo).
pub async fn esegui_dns_leak_test(client: &Client, n: usize) -> Vec<DnsServerEntry> {
    let mut set = JoinSet::new();
    for _ in 0..n {
        set.spawn(query_singola(client.clone()));
    }
    let mut trovati: IndexMap<String, DnsServerEntry> = IndexMap::new();
    while let Some(res) = set.join_next().await {
        if let Ok(Some(entry)) = res {
            unisci_resolver(&mut trovati, entry);
        }
    }
    trovati.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn prefisso_lungo_32_e_alfanumerico() {
        let a = prefisso_casuale();
        let b = prefisso_casuale();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert_ne!(a, b);
    }

    #[test]
    fn parse_edns_con_ecs() {
        let d = json!({
            "dns": {"geo": "Italy - Google", "ip": "74.125.1.1"},
            "edns": {"geo": "Italy - Fastweb", "ip": "203.0.113.0"}
        });
        let e = parse_edns(&d).unwrap();
        assert_eq!(e.ip, "74.125.1.1");
        assert_eq!(e.geo_org, "Italy - Google");
        assert_eq!(e.edns_ip.as_deref(), Some("203.0.113.0"));
        assert_eq!(e.edns_geo_org.as_deref(), Some("Italy - Fastweb"));
    }

    #[test]
    fn parse_edns_senza_ecs_o_stringa() {
        let d = json!({"dns": {"ip": "1.1.1.1"}, "edns": "None"});
        let e = parse_edns(&d).unwrap();
        assert_eq!(e.geo_org, "Sconosciuto");
        assert!(e.edns_ip.is_none() && e.edns_geo_org.is_none());
        assert!(parse_edns(&json!({"dns": {}})).is_none());
        assert!(parse_edns(&json!({"dns": "x"})).is_none());
        assert!(parse_edns(&json!({})).is_none());
    }

    #[test]
    fn unisci_completa_ecs_mancante() {
        let mut m = IndexMap::new();
        let base = DnsServerEntry {
            ip: "9.9.9.9".into(),
            geo_org: "Quad9".into(),
            edns_ip: None,
            edns_geo_org: None,
            is_leak_suspect: false,
        };
        unisci_resolver(&mut m, base.clone());
        unisci_resolver(&mut m, base.clone());
        assert_eq!(m.len(), 1);
        let mut con_ecs = base.clone();
        con_ecs.edns_ip = Some("10.0.0.0".into());
        con_ecs.edns_geo_org = Some("X".into());
        unisci_resolver(&mut m, con_ecs);
        assert_eq!(m["9.9.9.9"].edns_ip.as_deref(), Some("10.0.0.0"));
        assert_eq!(m.len(), 1);
    }
}
