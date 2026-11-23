//! Test della logica del verdetto con report finti.

use indexmap::IndexMap;

use super::blacklist::{StatoDnsbl, VoceDnsbl};
use super::ip::{Ipv4Details, MullvadInfo};
use super::report::*;
use super::rete_locale::LocalNetwork;

fn tunnel(nome: &str, ipv6: &str) -> TunnelInterface {
    TunnelInterface {
        name: nome.into(),
        ipv4: "10.64.0.2".into(),
        ipv6: ipv6.into(),
        mtu: 1380,
        is_up: true,
        speed_mbps: 0,
        is_tunnel: true,
    }
}

fn resolver(ip: &str, geo: &str, ecs: Option<&str>) -> DnsServerEntry {
    DnsServerEntry {
        ip: ip.into(),
        geo_org: geo.into(),
        edns_ip: ecs.map(str::to_string),
        edns_geo_org: ecs.map(|_| "x".to_string()),
        is_leak_suspect: false,
    }
}

fn blacklist(listati: &[&str]) -> IndexMap<String, VoceDnsbl> {
    ["Spamhaus ZEN", "SpamCop", "Barracuda", "SORBS DNSBL", "UCEPROTECT L1", "Backscatterer"]
        .iter()
        .map(|n| {
            let stato = if listati.contains(n) { StatoDnsbl::InLista } else { StatoDnsbl::Pulito };
            (n.to_string(), VoceDnsbl::nuova(stato, None))
        })
        .collect()
}

struct Scenario {
    ipv4: Ipv4Details,
    mullvad: MullvadInfo,
    ipv6: Option<String>,
    dns: Vec<DnsServerEntry>,
    stun: Option<String>,
    locale: LocalNetwork,
    blacklist: IndexMap<String, VoceDnsbl>,
}

fn ip_isp_domestico() -> Ipv4Details {
    Ipv4Details {
        ipv4: Some("203.0.113.7".into()),
        isp: "Fastweb".into(),
        org: "Fastweb S.p.A.".into(),
        asn: "AS12874 Fastweb".into(),
        country: "Italy".into(),
        city: "Roma".into(),
        proxy: Some(false),
        hosting: Some(false),
        ..Default::default()
    }
}

fn ip_vpn() -> Ipv4Details {
    Ipv4Details {
        ipv4: Some("198.51.100.4".into()),
        isp: "31173 Services AB".into(),
        org: "Mullvad VPN".into(),
        asn: "AS39351 31173 Services AB".into(),
        country: "Sweden".into(),
        city: "Stockholm".into(),
        proxy: Some(false),
        hosting: Some(false),
        ..Default::default()
    }
}

fn base(ipv4: Ipv4Details) -> Scenario {
    Scenario {
        ipv4,
        mullvad: MullvadInfo::default(),
        ipv6: None,
        dns: vec![resolver("9.9.9.9", "Switzerland - Quad9", None)],
        stun: None,
        locale: LocalNetwork::default(),
        blacklist: blacklist(&[]),
    }
}

fn valuta(s: Scenario) -> SecurityReport {
    let stun = s.stun.or_else(|| s.ipv4.ipv4.clone());
    costruisci_report(
        DatiRaccolti {
            ipv4: s.ipv4,
            mullvad: s.mullvad,
            ipv6: s.ipv6,
            dns_resolvers: s.dns,
            stun_ipv4: stun,
            latency_ms: Some(20.0),
            locale: s.locale,
            blacklist: s.blacklist,
        },
        "2026-11-23T10:00:00+01:00".into(),
        1.0,
    )
}

fn con_tunnel(s: &mut Scenario, route: bool, strict: bool) {
    s.locale.tunnels = vec![tunnel("Mullvad", "Disabilitato/Link-Local")];
    s.locale.all_up = s.locale.tunnels.clone();
    s.locale.default_route_via_tunnel = route;
    s.locale.killswitch_or_strict_route = strict;
}

#[test]
fn senza_ipv4_e_errore() {
    let r = valuta(base(Ipv4Details::default()));
    assert_eq!(r.overall_status, OverallStatus::Error);
    assert_eq!(r.overall_title, "ERRORE DI CONNESSIONE");
    assert_eq!(r.security_score, 0);
    assert!(r.warnings.is_empty() && r.positives.is_empty());
}

#[test]
fn senza_vpn_e_non_protetto() {
    let r = valuta(base(ip_isp_domestico()));
    assert_eq!(r.overall_status, OverallStatus::Unprotected);
    assert!(!r.is_vpn_or_proxy_ip);
    // dns 15 + stun 10 + ipv6 assente 5 + blacklist 5
    assert_eq!(r.security_score, 35);
    assert!(r.warnings[0].starts_with("NON SEI SU VPN"));
    assert_eq!(r.dns_status_text, "Nessun DNS Leak rilevato (1 server DNS verificati)");
}

#[test]
fn vpn_completa_e_protetta_con_100() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    let r = valuta(s);
    assert_eq!(r.overall_status, OverallStatus::Protected);
    assert!(r.is_hosting_datacenter, "keyword mullvad/31173 nel testo di rete");
    // 50 + 10 + 5 + 15 + 10 + 5 + 5
    assert_eq!(r.security_score, 100);
    assert!(r.warnings.is_empty());
    assert_eq!(r.overall_title, "CONNESSIONE PROTETTA (VPN SICURA)");
}

#[test]
fn ip_vpn_senza_tunnel_locale_e_warning() {
    let r = valuta(base(ip_vpn()));
    assert_eq!(r.overall_status, OverallStatus::Warning);
    assert_eq!(r.overall_title, "CONNESSIONE VPN ATTIVA (CON AVVISI)");
    // 40 + 15 + 10 + 5 + 5
    assert_eq!(r.security_score, 75);
    assert_eq!(r.warnings.len(), 1);
    assert!(r.overall_summary.contains("1 avvisi non critici"));
}

#[test]
fn tunnel_ma_ip_isp_standard() {
    let mut s = base(ip_isp_domestico());
    con_tunnel(&mut s, false, false);
    let r = valuta(s);
    // tunnel attivo ma IP dell'operatore e nessun leak: Warning dedicato (split tunneling)
    assert!(r.is_vpn_or_proxy_ip);
    assert_eq!(r.security_score, 20 + 15 + 10 + 5 + 5);
    assert!(r.warnings[0].starts_with("Interfaccia VPN locale attiva (Mullvad)"));
    assert_eq!(r.overall_status, OverallStatus::Warning);
    assert_eq!(r.overall_title, "Tunnel attivo, traffico non protetto");
    assert_eq!(
        r.overall_summary,
        "È attiva un'interfaccia VPN/tunnel, ma il traffico Internet esce dall'IP del tuo operatore (split tunneling o VPN senza uscita, es. Tailscale)."
    );
}

#[test]
fn dns_leak_isp_domestico_con_vpn() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.dns = vec![resolver("1.2.3.4", "Italy - Telecom Italia", None)];
    let r = valuta(s);
    assert!(r.dns_leak_detected && r.dns_resolvers[0].is_leak_suspect);
    assert_eq!(r.overall_status, OverallStatus::Warning);
    assert_eq!(r.overall_title, "ATTENZIONE: VPN ATTIVA MA CON LEAK RILEVATI!");
    // 50 + 10 + 5 - 20 + 10 + 5 + 5
    assert_eq!(r.security_score, 65);
}

#[test]
fn edns_ecs_da_avviso_non_critico() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.dns = vec![resolver("8.8.8.8", "Google", Some("203.0.113.0"))];
    let r = valuta(s);
    assert!(r.edns_leak_detected && !r.dns_leak_detected);
    assert_eq!(r.overall_status, OverallStatus::Warning);
    assert!(r.dns_status_text.starts_with("AVVISO EDNS"));
    // 50 + 10 + 5 + 5 + 10 + 5 + 5
    assert_eq!(r.security_score, 90);
}

#[test]
fn webrtc_leak_se_stun_diverso() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.stun = Some("203.0.113.99".into());
    let r = valuta(s);
    assert!(r.webrtc_leak_detected);
    assert_eq!(
        r.webrtc_status_text,
        "LEAK RILEVATO! STUN espone IP diverso: 203.0.113.99"
    );
    assert_eq!(r.overall_status, OverallStatus::Warning);
    // 50 + 10 + 5 + 15 - 25 + 5 + 5
    assert_eq!(r.security_score, 65);
}

#[test]
fn stun_bloccato_e_sicuro() {
    let mut r = SecurityReport::vuoto("t".into());
    r.ipv4 = Some("1.2.3.4".into());
    analizza_stun(&mut r);
    assert!(!r.webrtc_leak_detected);
    assert!(r.webrtc_status_text.starts_with("Sicuro"));
    r.stun_ipv4 = Some("1.2.3.4".into());
    analizza_stun(&mut r);
    assert!(r.webrtc_status_text.starts_with("Nessun leak"));
}

#[test]
fn ipv6_leak_con_tunnel_senza_v6() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.ipv6 = Some("2001:db8::99".into());
    let r = valuta(s);
    assert!(r.ipv6_leak_detected);
    assert_eq!(r.overall_status, OverallStatus::Warning);
    // 50 + 10 + 5 + 15 + 10 - 20 + 0 + 5
    assert_eq!(r.security_score, 75);
}

#[test]
fn ipv6_non_e_leak_se_il_tunnel_ha_v6() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.locale.tunnels[0].ipv6 = "2001:db8::2".into();
    s.ipv6 = Some("2001:db8::99".into());
    let r = valuta(s);
    assert!(!r.ipv6_leak_detected);
    assert_eq!(r.overall_status, OverallStatus::Protected);
}

#[test]
fn blacklist_porta_a_warning_e_avviso() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.blacklist = blacklist(&["Spamhaus ZEN", "SpamCop"]);
    let r = valuta(s);
    assert_eq!(r.blacklisted_count, 2);
    assert_eq!(r.overall_status, OverallStatus::Warning);
    assert!(r.warnings.iter().any(|w| w.contains("segnalato in 2 blacklist (Spamhaus ZEN, SpamCop)")));
    // 50 + 10 + 5 + 15 + 10 + 5 + 0
    assert_eq!(r.security_score, 95);
}

#[test]
fn mullvad_blacklisted_conta_come_blacklist() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.mullvad = MullvadInfo { is_mullvad_ip: true, host: "se-sto-wg-001".into(), blacklisted: Some(true) };
    let r = valuta(s);
    assert!(r.is_mullvad_server);
    assert_eq!(r.mullvad_server_name, "se-sto-wg-001");
    assert!(r.warnings.iter().any(|w| w == "IP segnalato da Mullvad come in blacklist"));
    assert!(!r.warnings.iter().any(|w| w.contains("0 blacklist")));
    // Conta come una blacklist: niente +5 e stato Warning
    assert_eq!(r.security_score, 95);
    assert_eq!(r.overall_status, OverallStatus::Warning);
}

#[test]
fn flag_proxy_dell_api_basta_senza_keyword() {
    let mut ip = ip_isp_domestico();
    ip.isp = "Acme Transit".into();
    ip.org = "Acme".into();
    ip.asn = "AS64500 Acme".into();
    ip.proxy = Some(true);
    let r = valuta(base(ip));
    assert!(r.is_vpn_or_proxy_ip && !r.is_hosting_datacenter);
    assert_eq!(r.overall_status, OverallStatus::Warning);
}

#[test]
fn nessun_resolver_dns_trovato() {
    let mut s = base(ip_vpn());
    s.dns = vec![];
    let r = valuta(s);
    assert_eq!(r.dns_status_text, "Impossibile determinare i resolver DNS esterni");
    // niente +15 per il DNS: 40 + 10 + 5 + 5
    assert_eq!(r.security_score, 60);
}

#[test]
fn punteggio_limitato_tra_0_e_100() {
    let mut s = base(ip_isp_domestico());
    con_tunnel(&mut s, false, false);
    s.dns = vec![resolver("1.2.3.4", "Vodafone Italia", None)];
    s.stun = Some("203.0.113.99".into());
    s.ipv6 = Some("2001:db8::1".into());
    let r = valuta(s);
    assert_eq!(r.security_score, 0);
}

#[test]
fn serializzazione_camel_case() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    let r = valuta(s);
    let v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["overallStatus"], "PROTECTED");
    assert_eq!(v["securityScore"], 100);
    assert_eq!(v["activeTunnels"][0]["isTunnel"], true);
    assert_eq!(v["dnsResolvers"][0]["geoOrg"], "Switzerland - Quad9");
    assert_eq!(v["blacklistResults"]["Spamhaus ZEN"]["stato"], "pulito");
    assert!(v["stunIpv4"].is_string());
    assert!(v["mullvadBlacklisted"].is_null());
    assert_eq!(v["timestamp"], "2026-11-23T10:00:00+01:00");
}

#[test]
fn liste_non_verificabili_e_pbl_non_contano() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    let mut bl = blacklist(&[]);
    bl["Spamhaus ZEN"] = VoceDnsbl::nuova(StatoDnsbl::Pulito, Some("PBL".into()));
    bl["SpamCop"] = VoceDnsbl::nuova(StatoDnsbl::NonVerificabile, Some("127.255.255.254".into()));
    s.blacklist = bl;
    let r = valuta(s);
    assert_eq!(r.blacklisted_count, 0);
    assert_eq!(r.overall_status, OverallStatus::Protected);
    assert_eq!(r.security_score, 100);
    assert!(
        r.positives
            .iter()
            .any(|p| p == "Indirizzo IP pulito (0 su 5 liste di blocco DNSBL verificate, 1 non verificabili).")
    );
}

// --- Lotto 5b ---------------------------------------------------------------------

#[test]
fn parole_chiave_per_parola_intera() {
    for no in ["Optimum", "Maritime", "Olimpia", "Estimated", "Attimo"] {
        assert!(!contiene_parola_chiave(no, DOMESTIC_ISPS), "{no} non deve combaciare con i domestici");
        assert!(!contiene_parola_chiave(no, VPN_KEYWORDS), "{no} non deve combaciare con le VPN");
    }
    assert!(contiene_parola_chiave("Telecom Italia (TIM)", DOMESTIC_ISPS));
    assert!(contiene_parola_chiave("Italy - Sky Italia", DOMESTIC_ISPS));
    assert!(contiene_parola_chiave("Wind Tre S.p.A.", DOMESTIC_ISPS), "sequenza di parole");
    assert!(!contiene_parola_chiave("Wind Energy Tre", DOMESTIC_ISPS), "le parole devono essere consecutive");
    assert!(contiene_parola_chiave("OVH SAS", VPN_KEYWORDS));
    assert!(!contiene_parola_chiave("Olimpia Hosting", VPN_KEYWORDS));
    assert!(contiene_parola_chiave("AS39351 31173 Services AB", VPN_KEYWORDS));
    assert!(contiene_parola_chiave("Private Internet Access, Inc.", VPN_KEYWORDS));
    assert!(contiene_parola_chiave("host.hide.me", VPN_KEYWORDS), "hide.me = hide + me");
    assert!(contiene_parola_chiave("Latitude.sh", VPN_KEYWORDS));
    assert!(!contiene_parola_chiave("", VPN_KEYWORDS));
}

#[test]
fn isp_con_nome_simile_a_una_parola_chiave_non_e_una_vpn() {
    let mut ip = ip_isp_domestico();
    ip.isp = "Optimum Online".into();
    ip.org = "Maritime Telecom Olimpia".into();
    let r = valuta(base(ip));
    assert!(!r.is_hosting_datacenter && !r.is_vpn_or_proxy_ip);
    assert_eq!(r.overall_status, OverallStatus::Unprotected);
}

#[test]
fn resolver_optimum_non_e_un_isp_domestico() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.dns = vec![resolver("1.2.3.4", "United States - Optimum", None)];
    let r = valuta(s);
    assert!(!r.dns_leak_detected);
    assert_eq!(r.overall_status, OverallStatus::Protected);
}

#[test]
fn resolver_tim_e_telecom_italia_sono_leak_con_vpn() {
    for geo in ["Italy - Telecom Italia", "Italy - TIM SpA", "Sky Italia"] {
        let mut s = base(ip_vpn());
        con_tunnel(&mut s, true, true);
        s.dns = vec![resolver("1.2.3.4", geo, None)];
        assert!(valuta(s).dns_leak_detected, "{geo}");
    }
}

#[test]
fn dns_non_verificabile_non_puo_essere_protetta() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.dns = vec![];
    let r = valuta(s);
    assert_eq!(r.overall_status, OverallStatus::Warning);
    assert!(r.warnings.iter().any(|w| w == "Test DNS leak non riuscito: impossibile verificare i DNS"));
    assert!(!r.overall_summary.to_lowercase().contains("nessun leak"), "{}", r.overall_summary);
    assert!(!r.positives.iter().any(|p| p.contains("Nessun DNS Leak")));
    // 50 + 10 + 5 + 0 (DNS) + 10 + 5 + 5
    assert_eq!(r.security_score, 85);
}

#[test]
fn vpn_ignota_con_ipwho_non_e_ne_vpn_ne_non_vpn() {
    let mut ip = ip_isp_domestico();
    ip.proxy = None; // fallback ipwho.is: non sa nulla di proxy/hosting
    ip.hosting = None;
    let r = valuta(base(ip));
    assert!(r.vpn_sconosciuta);
    assert!(!r.is_vpn_or_proxy_ip);
    assert_eq!(r.overall_status, OverallStatus::Warning);
    assert_eq!(r.overall_title, "STATO VPN NON VERIFICABILE");
    assert!(r.warnings.iter().any(|w| w.starts_with("Stato VPN sconosciuto")));
    assert!(!r.warnings.iter().any(|w| w.starts_with("NON SEI SU VPN")));
}

#[test]
fn vpn_ignota_si_risolve_con_un_segnale_positivo() {
    // Tunnel locale: lo stato lo decide il tunnel, non e` ignoto
    let mut ip = ip_isp_domestico();
    ip.proxy = None;
    ip.hosting = None;
    let mut s = base(ip);
    con_tunnel(&mut s, false, false);
    assert!(!valuta(s).vpn_sconosciuta);
    // Parola chiave di un provider VPN nell'ISP: noto
    let mut ip = ip_vpn();
    ip.proxy = None;
    ip.hosting = None;
    let r = valuta(base(ip));
    assert!(!r.vpn_sconosciuta && r.is_hosting_datacenter);
    // ip-api con flag falsi: noto, non VPN
    let r = valuta(base(ip_isp_domestico()));
    assert!(!r.vpn_sconosciuta);
}

#[test]
fn serializzazione_vpn_sconosciuta_con_default_per_i_vecchi_report() {
    let r = valuta(base(Ipv4Details { ipv4: Some("1.2.3.4".into()), ..Default::default() }));
    let mut v = serde_json::to_value(&r).unwrap();
    assert_eq!(v["vpnSconosciuta"], true);
    v.as_object_mut().unwrap().remove("vpnSconosciuta");
    let letto: SecurityReport = serde_json::from_value(v).unwrap();
    assert!(!letto.vpn_sconosciuta, "report salvati prima del campo si leggono ancora");
}

#[test]
fn mullvad_segnala_e_dnsbl_in_lista_insieme() {
    let mut s = base(ip_vpn());
    con_tunnel(&mut s, true, true);
    s.blacklist = blacklist(&["Spamhaus ZEN"]);
    s.mullvad = MullvadInfo { is_mullvad_ip: true, host: "h".into(), blacklisted: Some(true) };
    let r = valuta(s);
    assert!(r.warnings.iter().any(|w| w.contains("segnalato in 1 blacklist (Spamhaus ZEN) e da Mullvad")));
}

/// Le due lingue danno testi diversi e non vuoti per un verdetto, un errore e gli avvisi.
#[test]
fn verdetti_nelle_due_lingue() {
    use crate::testi::{LinguaEffettiva::{En, It}, con_lingua};
    let coppia = |f: &dyn Fn() -> SecurityReport| (con_lingua(It, f), con_lingua(En, f));

    let (it, en) = coppia(&|| valuta(base(Ipv4Details::default())));
    assert_eq!((it.overall_title.as_str(), en.overall_title.as_str()), ("ERRORE DI CONNESSIONE", "CONNECTION ERROR"));
    assert!(!it.overall_summary.is_empty() && !en.overall_summary.is_empty());
    assert_ne!(it.overall_summary, en.overall_summary);

    let (it, en) = coppia(&|| valuta(base(ip_isp_domestico())));
    assert_eq!(it.overall_status, en.overall_status, "il verdetto non dipende dalla lingua");
    assert!(it.overall_title.contains("NON PROTETTA") && en.overall_title.contains("NOT PROTECTED"));
    assert_eq!(it.warnings.len(), en.warnings.len());
    for (a, b) in it.warnings.iter().zip(&en.warnings) {
        assert!(!a.is_empty() && !b.is_empty());
        assert_ne!(a, b);
    }
    assert_ne!(it.dns_status_text, en.dns_status_text);
    assert_ne!(it.webrtc_status_text, en.webrtc_status_text);
}
