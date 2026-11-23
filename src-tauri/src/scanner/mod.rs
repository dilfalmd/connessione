//! Scanner di sicurezza della connessione / VPN: porting in Rust di `scanner.py`.
//!
//! I controlli (IP, IPv6, DNS leak, STUN, latenza, rete locale) girano in parallelo con tokio;
//! la blacklist DNSBL parte appena e` noto l'IPv4 pubblico.

pub mod blacklist;
pub mod dns_leak;
pub mod ip;
pub mod latenza;
pub mod report;
pub mod rete_locale;
pub mod stun;

use std::time::Instant;

use chrono::{Local, SecondsFormat};
use reqwest::Client;

pub use report::SecurityReport;

use report::{DatiRaccolti, costruisci_report};
use crate::testi::tr;

const USER_AGENT: &str = "VPN-Security-Checker/1.0";

/// Esegue tutti i controlli e costruisce il `SecurityReport` completo.
pub async fn esegui_scansione() -> Result<SecurityReport, String> {
    let inizio = Instant::now();
    // Radici CA incorporate (come certifi nel Python): con il verificatore di piattaforma di
    // Windows ogni nuovo handshake TLS costa ~2 s in piu`, che manda in timeout i servizi.
    let radici: Vec<reqwest::Certificate> = webpki_root_certs::TLS_SERVER_ROOT_CERTS
        .iter()
        .filter_map(|c| reqwest::Certificate::from_der(c.as_ref()).ok())
        .collect();
    let client = Client::builder()
        .user_agent(USER_AGENT)
        .tls_certs_only(radici)
        .build()
        .map_err(|e| tr!("Impossibile creare il client HTTP: {e}", "Unable to create the HTTP client: {e}"))?;

    let ip_e_blacklist = async {
        let (dettagli, mullvad) = ip::fetch_ipv4_details(&client).await;
        let blacklist = blacklist::controlla_tutte(dettagli.ipv4.as_deref()).await;
        (dettagli, mullvad, blacklist)
    };

    let (
        (ipv4, mullvad, blacklist),
        ipv6,
        dns_resolvers,
        stun_ipv4,
        latency_ms,
        locale,
    ) = tokio::join!(
        ip_e_blacklist,
        ip::fetch_ipv6(&client),
        dns_leak::esegui_dns_leak_test(&client, dns_leak::NUM_QUERIES),
        stun::query_stun_udp_async(),
        latenza::misura_latenza(),
        rete_locale::ispeziona_rete_locale_async(),
    );

    let timestamp = Local::now().to_rfc3339_opts(SecondsFormat::Secs, false);
    let durata = (inizio.elapsed().as_secs_f64() * 100.0).round() / 100.0;

    Ok(costruisci_report(
        DatiRaccolti {
            ipv4,
            mullvad,
            ipv6,
            dns_resolvers,
            stun_ipv4,
            latency_ms,
            locale,
            blacklist,
        },
        timestamp,
        durata,
    ))
}

#[cfg(test)]
mod test_verdetto;

#[cfg(test)]
mod test_integrazione {
    use super::*;

    /// Scan reale: `cargo test -- --ignored --nocapture`
    #[tokio::test]
    #[ignore = "esegue lo scan reale sulla rete"]
    async fn scan_reale() {
        let r = esegui_scansione().await.expect("scan fallito");
        println!("--- RIASSUNTO SCAN REALE ---");
        println!("timestamp      : {}", r.timestamp);
        println!("durata         : {} s", r.duration_sec);
        println!("stato          : {:?} (punteggio {})", r.overall_status, r.security_score);
        println!("titolo         : {}", r.overall_title);
        println!("ipv4           : {:?}", r.ipv4);
        println!("ipv6           : {:?}", r.ipv6);
        println!("isp/org/asn    : {} | {} | {}", r.isp, r.org, r.asn);
        println!("posizione      : {}, {}, {} ({}) [{}]", r.city, r.region, r.country, r.country_code, r.coordinates);
        println!("timezone       : {}", r.timezone);
        println!("reverse dns    : {}", r.reverse_dns);
        println!("vpn/proxy ip   : {} | datacenter: {} | mullvad: {} ({})", r.is_vpn_or_proxy_ip, r.is_hosting_datacenter, r.is_mullvad_server, r.mullvad_server_name);
        println!("tunnel attivi  : {:?}", r.active_tunnels.iter().map(|t| &t.name).collect::<Vec<_>>());
        println!("interfacce up  : {:?}", r.all_up_interfaces.iter().map(|t| (&t.name, &t.ipv4, t.mtu, t.is_tunnel)).collect::<Vec<_>>());
        println!("dns locali     : {:?}", r.local_dns_servers);
        println!("route tunnel   : {} | killswitch: {}", r.default_route_via_tunnel, r.killswitch_or_strict_route_detected);
        println!("dns resolver   : {:?}", r.dns_resolvers.iter().map(|d| (&d.ip, &d.geo_org, &d.edns_ip)).collect::<Vec<_>>());
        println!("dns leak/edns  : {} / {} -> {}", r.dns_leak_detected, r.edns_leak_detected, r.dns_status_text);
        println!("stun           : {:?} -> {}", r.stun_ipv4, r.webrtc_status_text);
        println!("ipv6 leak      : {}", r.ipv6_leak_detected);
        println!("latenza        : {:?} ms", r.latency_ms);
        for (nome, v) in &r.blacklist_results { println!("dnsbl {nome:<14}: {:?} {:?}", v.stato, v.nota); }
        println!("blacklist      : in lista {} (mullvad: {:?})", r.blacklisted_count, r.mullvad_blacklisted);
        for w in &r.warnings { println!("AVVISO  : {w}"); }
        for p in &r.positives { println!("POSITIVO: {p}"); }
        println!("json report ok : {} byte", serde_json::to_string(&r).unwrap().len());
        assert!(!r.timestamp.is_empty());
    }
}
