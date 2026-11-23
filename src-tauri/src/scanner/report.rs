//! Struttura dati `SecurityReport` e logica che produce punteggio, avvisi e verdetto finale
//! (porting di `run_full_security_scan` di scanner.py, parte successiva alla raccolta dati).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::blacklist::{StatoDnsbl, VoceDnsbl};
use super::ip::{Ipv4Details, MullvadInfo};
use super::rete_locale::LocalNetwork;
use crate::testi::{sc, tr};

/// Parole chiave tipiche di provider VPN, hosting o tunnel (applicate a ISP/org/ASN/reverse DNS).
pub const VPN_KEYWORDS: &[&str] = &[
    "mullvad", "nordvpn", "nordlynx", "proton", "protonvpn", "expressvpn",
    "surfshark", "cyberghost", "pia", "private internet access", "windscribe",
    "ivpn", "airvpn", "hide.me", "perfect privacy", "vyprvpn", "torguard",
    "openvpn", "wireguard", "wintun", "tap-windows", "tap0901", "tailscale",
    "zerotier", "cloudflare warp", "warp", "tzulo", "m247", "datapacket",
    "choopa", "vultr", "digitalocean", "linode", "akamai", "hetzner", "ovh",
    "leaseweb", "datacamp", "cdn77", "tefincom", "packethub", "blix",
    "31173", "xtom", "worldstream", "quadranet", "cogent", "velia", "latitude.sh",
];

/// ISP domestici italiani tipici: un resolver di questi con VPN attiva e` un possibile leak DNS.
pub const DOMESTIC_ISPS: &[&str] = &[
    "telecom italia", "tim", "vodafone", "fastweb", "wind tre", "tiscali", "sky", "eolo",
    "iliad", "regione",
];

/// Divide in parole (minuscole, solo caratteri alfanumerici).
fn tokenizza(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Vero se una delle parole chiave compare nel testo come parola intera; le parole chiave
/// composte ("private internet access", "hide.me") come sequenza di parole. Evita i falsi
/// positivi di una ricerca per sottostringa ("tim" in "Optimum", "ovh" in "Olimpia").
pub fn contiene_parola_chiave(testo: &str, parole_chiave: &[&str]) -> bool {
    let token = tokenizza(testo);
    parole_chiave.iter().any(|k| {
        let kt = tokenizza(k);
        !kt.is_empty() && token.windows(kt.len()).any(|w| w == kt.as_slice())
    })
}

pub const IPV6_PLACEHOLDER: &str = "Disabilitato/Link-Local";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OverallStatus {
    Protected,
    Warning,
    Unprotected,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInterface {
    pub name: String,
    pub ipv4: String,
    pub ipv6: String,
    pub mtu: u32,
    pub is_up: bool,
    pub speed_mbps: u64,
    pub is_tunnel: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsServerEntry {
    pub ip: String,
    pub geo_org: String,
    pub edns_ip: Option<String>,
    pub edns_geo_org: Option<String>,
    pub is_leak_suspect: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityReport {
    /// Data/ora locale in formato ISO 8601 (RFC 3339).
    pub timestamp: String,
    pub duration_sec: f64,

    pub overall_status: OverallStatus,
    pub overall_title: String,
    pub overall_summary: String,
    /// 0 - 100
    pub security_score: i32,

    // 1. IP, geolocalizzazione, VPN/proxy
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub isp: String,
    pub org: String,
    pub asn: String,
    pub country: String,
    pub country_code: String,
    pub region: String,
    pub city: String,
    pub timezone: String,
    pub coordinates: String,
    pub reverse_dns: String,

    pub is_vpn_or_proxy_ip: bool,
    /// Il servizio IP non ha detto se e` una VPN/proxy e nient'altro lo decide: stato ignoto.
    #[serde(default)]
    pub vpn_sconosciuta: bool,
    pub is_hosting_datacenter: bool,
    pub is_mullvad_server: bool,
    pub mullvad_server_name: String,

    // 2. Tunnel e interfacce locali
    pub active_tunnels: Vec<TunnelInterface>,
    pub all_up_interfaces: Vec<TunnelInterface>,
    pub local_dns_servers: Vec<String>,
    pub default_route_via_tunnel: bool,
    pub killswitch_or_strict_route_detected: bool,

    // 3. DNS leak ed EDNS
    pub dns_resolvers: Vec<DnsServerEntry>,
    pub dns_leak_detected: bool,
    pub edns_leak_detected: bool,
    pub dns_status_text: String,

    // 4. WebRTC (STUN) e latenza
    pub stun_ipv4: Option<String>,
    pub webrtc_leak_detected: bool,
    pub webrtc_status_text: String,
    pub ipv6_leak_detected: bool,
    pub latency_ms: Option<f64>,

    // 5. Blacklist (DNSBL)
    /// Stato per lista; solo `in_lista` conta per punteggio e verdetto.
    pub blacklist_results: IndexMap<String, VoceDnsbl>,
    pub blacklisted_count: usize,
    pub mullvad_blacklisted: Option<bool>,

    pub warnings: Vec<String>,
    pub positives: Vec<String>,
}

impl SecurityReport {
    /// Report vuoto, equivalente ai valori di default del dataclass Python.
    pub fn vuoto(timestamp: String) -> Self {
        SecurityReport {
            timestamp,
            duration_sec: 0.0,
            overall_status: OverallStatus::Unprotected,
            overall_title: "Controllo non eseguito".into(),
            overall_summary: String::new(),
            security_score: 0,
            ipv4: None,
            ipv6: None,
            isp: "Sconosciuto".into(),
            org: "Sconosciuto".into(),
            asn: "Sconosciuto".into(),
            country: "Sconosciuto".into(),
            country_code: String::new(),
            region: String::new(),
            city: String::new(),
            timezone: String::new(),
            coordinates: String::new(),
            reverse_dns: String::new(),
            is_vpn_or_proxy_ip: false,
            vpn_sconosciuta: false,
            is_hosting_datacenter: false,
            is_mullvad_server: false,
            mullvad_server_name: String::new(),
            active_tunnels: Vec::new(),
            all_up_interfaces: Vec::new(),
            local_dns_servers: Vec::new(),
            default_route_via_tunnel: false,
            killswitch_or_strict_route_detected: false,
            dns_resolvers: Vec::new(),
            dns_leak_detected: false,
            edns_leak_detected: false,
            dns_status_text: String::new(),
            stun_ipv4: None,
            webrtc_leak_detected: false,
            webrtc_status_text: String::new(),
            ipv6_leak_detected: false,
            latency_ms: None,
            blacklist_results: IndexMap::new(),
            blacklisted_count: 0,
            mullvad_blacklisted: None,
            warnings: Vec::new(),
            positives: Vec::new(),
        }
    }
}

/// Dati grezzi raccolti dai controlli paralleli, prima dell'analisi.
pub struct DatiRaccolti {
    pub ipv4: Ipv4Details,
    pub mullvad: MullvadInfo,
    pub ipv6: Option<String>,
    pub dns_resolvers: Vec<DnsServerEntry>,
    pub stun_ipv4: Option<String>,
    pub latency_ms: Option<f64>,
    pub locale: LocalNetwork,
    pub blacklist: IndexMap<String, VoceDnsbl>,
}

/// Costruisce il report completo dai dati raccolti: popola i campi, esegue le analisi
/// (hosting/VPN, STUN, IPv6, DNS) e calcola punteggio e verdetto.
pub fn costruisci_report(dati: DatiRaccolti, timestamp: String, duration_sec: f64) -> SecurityReport {
    let mut report = SecurityReport::vuoto(timestamp);
    let proxy_flag = dati.ipv4.proxy;
    let hosting_flag = dati.ipv4.hosting;

    report.ipv6 = dati.ipv6;
    report.dns_resolvers = dati.dns_resolvers;
    report.stun_ipv4 = dati.stun_ipv4;
    report.latency_ms = dati.latency_ms;
    report.active_tunnels = dati.locale.tunnels;
    report.all_up_interfaces = dati.locale.all_up;
    report.local_dns_servers = dati.locale.dns_servers;
    report.default_route_via_tunnel = dati.locale.default_route_via_tunnel;
    report.killswitch_or_strict_route_detected = dati.locale.killswitch_or_strict_route;

    let d = dati.ipv4;
    report.ipv4 = d.ipv4;
    report.country = d.country;
    report.country_code = d.country_code;
    report.region = d.region;
    report.city = d.city;
    report.timezone = d.timezone;
    if let (Some(lat), Some(lon)) = (d.lat, d.lon) {
        report.coordinates = format!("{lat:.4}, {lon:.4}");
    }
    report.isp = d.isp;
    report.org = d.org;
    report.asn = d.asn;
    report.reverse_dns = d.reverse;
    report.is_mullvad_server = dati.mullvad.is_mullvad_ip;
    report.mullvad_server_name = dati.mullvad.host;
    report.mullvad_blacklisted = dati.mullvad.blacklisted;

    report.blacklisted_count = dati
        .blacklist
        .values()
        .filter(|v| v.stato == StatoDnsbl::InLista)
        .count();
    report.blacklist_results = dati.blacklist;

    analizza_ip_vpn(&mut report, proxy_flag, hosting_flag);
    analizza_stun(&mut report);
    analizza_ipv6(&mut report);
    analizza_dns(&mut report);
    calcola_verdetto(&mut report, proxy_flag == Some(true));

    report.duration_sec = duration_sec;
    report
}

/// Determina se l'IP appartiene a una VPN / proxy / datacenter.
/// `proxy_flag` e `hosting_flag` sono `None` quando il servizio non li fornisce (ignoto).
pub fn analizza_ip_vpn(report: &mut SecurityReport, proxy_flag: Option<bool>, hosting_flag: Option<bool>) {
    let combined = format!(
        "{} {} {} {}",
        report.isp, report.org, report.asn, report.reverse_dns
    );
    let keyword_match = contiene_parola_chiave(&combined, VPN_KEYWORDS);
    report.is_hosting_datacenter = hosting_flag == Some(true) || keyword_match;
    let ha_tunnel = !report.active_tunnels.is_empty();
    report.is_vpn_or_proxy_ip = proxy_flag == Some(true)
        || report.is_hosting_datacenter
        || report.is_mullvad_server
        || ha_tunnel;
    // Ignoto: nessun segnale positivo (tunnel locale, Mullvad, parola chiave, flag) e almeno
    // uno dei flag del servizio manca. Non conta ne` come VPN ne` come non-VPN.
    report.vpn_sconosciuta =
        !report.is_vpn_or_proxy_ip && (proxy_flag.is_none() || hosting_flag.is_none());
}

/// Analisi WebRTC (STUN) leak: confronta l'IP visto via UDP con quello HTTP.
pub fn analizza_stun(report: &mut SecurityReport) {
    match (&report.stun_ipv4, &report.ipv4) {
        (None, _) => {
            report.webrtc_leak_detected = false;
            report.webrtc_status_text = sc("Sicuro (Traffico UDP STUN bloccato o filtrato)", "Safe (UDP STUN traffic blocked or filtered)").into();
        }
        (Some(stun), Some(ip)) if stun != ip => {
            report.webrtc_leak_detected = true;
            report.webrtc_status_text = tr!("LEAK RILEVATO! STUN espone IP diverso: {stun}", "LEAK DETECTED! STUN exposes a different IP: {stun}");
        }
        (Some(stun), _) => {
            report.webrtc_leak_detected = false;
            report.webrtc_status_text =
                tr!("Nessun leak (IP STUN coincide con IP pubblico: {stun})", "No leak (STUN IP matches the public IP: {stun})");
        }
    }
}

/// Analisi IPv6 leak: IPv6 pubblico raggiungibile, tunnel attivo ma senza IPv6 globale.
pub fn analizza_ipv6(report: &mut SecurityReport) {
    if report.ipv6.is_some() && !report.active_tunnels.is_empty() {
        let tunnel_has_v6 = report
            .active_tunnels
            .iter()
            .any(|t| !t.ipv6.is_empty() && t.ipv6 != IPV6_PLACEHOLDER);
        if !tunnel_has_v6 {
            report.ipv6_leak_detected = true;
        }
    }
}

/// Analisi DNS leak ed EDNS Client Subnet.
pub fn analizza_dns(report: &mut SecurityReport) {
    if report.dns_resolvers.is_empty() {
        report.dns_status_text = sc("Impossibile determinare i resolver DNS esterni", "Unable to determine the external DNS resolvers").into();
        return;
    }
    let vpn = report.is_vpn_or_proxy_ip;
    let mut dns_leak = report.dns_leak_detected;
    let mut edns_leak = report.edns_leak_detected;
    for resolver in report.dns_resolvers.iter_mut() {
        // ECS: il resolver espone la sottorete del nostro IP reale o dell'ISP
        if resolver.edns_ip.is_some() {
            edns_leak = true;
            resolver.is_leak_suspect = true;
        }
        // Con VPN attiva, un resolver di un ISP domestico italiano e` un leak
        if vpn && contiene_parola_chiave(&resolver.geo_org, DOMESTIC_ISPS) {
            dns_leak = true;
            resolver.is_leak_suspect = true;
        }
    }
    report.dns_leak_detected = dns_leak;
    report.edns_leak_detected = edns_leak;

    report.dns_status_text = if dns_leak {
        sc("POSSIBILE DNS LEAK: Rilevati server DNS del fornitore Internet locale!", "POSSIBLE DNS LEAK: DNS servers of your local Internet provider detected!").into()
    } else if edns_leak {
        sc("AVVISO EDNS: Il server DNS trasmette la tua sottorete (EDNS Client Subnet attivo)", "EDNS WARNING: the DNS server passes on your subnet (EDNS Client Subnet is on)").into()
    } else {
        tr!(
            "Nessun DNS Leak rilevato ({} server DNS verificati)",
            "No DNS leak detected ({} DNS servers checked)",
            report.dns_resolvers.len()
        )
    };
}

/// Calcola punteggio (0-100), avvisi, aspetti positivi e stato complessivo.
/// `proxy_flag` e` il flag proxy restituito dal servizio IP (non e` un campo del report).
pub fn calcola_verdetto(report: &mut SecurityReport, proxy_flag: bool) {
    if report.ipv4.is_none() {
        report.overall_status = OverallStatus::Error;
        report.overall_title = sc("ERRORE DI CONNESSIONE", "CONNECTION ERROR").into();
        report.overall_summary = sc("Impossibile contattare i server di verifica. Controlla la connessione a Internet o il Kill Switch.", "Unable to reach the verification servers. Check your Internet connection or the Kill Switch.").into();
        return;
    }

    let mut score: i32 = 0;
    let has_tunnel = !report.active_tunnels.is_empty();
    let is_datacenter_or_vpn = proxy_flag || report.is_hosting_datacenter || report.is_mullvad_server;

    // Protezione VPN
    if has_tunnel && is_datacenter_or_vpn {
        score += 50;
        let nomi: Vec<&str> = report.active_tunnels.iter().map(|t| t.name.as_str()).collect();
        report.positives.push(tr!(
            "Tunnel VPN attivo ({}) e IP protetto ({}).",
            "Active VPN tunnel ({}) and protected IP ({}).",
            nomi.join(", "),
            report.isp
        ));
    } else if is_datacenter_or_vpn {
        score += 40;
        report.positives.push(tr!(
            "IP pubblico appartenente a rete VPN/Datacenter ({}).",
            "Public IP belongs to a VPN/datacenter network ({}).",
            report.isp
        ));
        report.warnings.push(
            sc("Nessuna interfaccia tunnel locale rilevata (la VPN potrebbe essere sul router o tramite proxy/estensione).", "No local tunnel interface detected (the VPN may be on the router or via a proxy/extension).")
                .into(),
        );
    } else if has_tunnel {
        score += 20;
        report.warnings.push(tr!(
            "Interfaccia VPN locale attiva ({}), ma l'IP pubblico ({}) sembra appartenere a un ISP standard ({})!",
            "Local VPN interface is up ({}), but the public IP ({}) looks like it belongs to a standard ISP ({})!",
            report.active_tunnels[0].name,
            report.ipv4.as_deref().unwrap_or(""),
            report.isp
        ));
    } else if report.vpn_sconosciuta {
        report.warnings.push(
            sc("Stato VPN sconosciuto: il servizio di verifica non ha fornito i dati su VPN/proxy, impossibile stabilire se il traffico è protetto.", "VPN status unknown: the lookup service gave no VPN/proxy data, so it cannot be determined whether your traffic is protected.")
                .into(),
        );
    } else {
        report.warnings.push(tr!(
            "NON SEI SU VPN: L'indirizzo IP ({}) appartiene al tuo ISP diretto ({}) e non ci sono tunnel VPN attivi.",
            "NOT ON A VPN: the IP address ({}) belongs to your direct ISP ({}) and there are no active VPN tunnels.",
            report.ipv4.as_deref().unwrap_or(""),
            report.isp
        ));
    }

    // Routing / kill switch
    if has_tunnel {
        if report.default_route_via_tunnel {
            score += 10;
            report.positives.push(
                sc("Il traffico predefinito (Default Gateway) è instradato correttamente nel tunnel VPN.", "Default traffic (default gateway) is correctly routed through the VPN tunnel.").into(),
            );
        }
        if report.killswitch_or_strict_route_detected {
            score += 5;
            report.positives.push(
                sc("Rilevate regole di routing stretto / protezione contro caduta accidentale del tunnel.", "Strict routing rules / protection against accidental tunnel drops detected.").into(),
            );
        }
    }

    // DNS
    if !report.dns_leak_detected && !report.edns_leak_detected && !report.dns_resolvers.is_empty() {
        score += 15;
        report
            .positives
            .push(sc("Nessun DNS Leak o esposizione EDNS Client Subnet rilevata.", "No DNS leak or EDNS Client Subnet exposure detected.").into());
    } else if report.dns_leak_detected {
        score -= 20;
        report.warnings.push(
            sc("DNS Leak rilevato: le tue richieste DNS passano per i server dell'ISP locale fuori dalla VPN!", "DNS leak detected: your DNS requests go through your local ISP's servers outside the VPN!").into(),
        );
    } else if report.edns_leak_detected {
        score += 5;
        report.warnings.push(
            sc("EDNS Client Subnet attivo: il tuo resolver DNS (es. Google DNS) comunica la tua sottorete geografica ai server autoritativi.", "EDNS Client Subnet is on: your DNS resolver (e.g. Google DNS) tells authoritative servers your network subnet.").into(),
        );
    } else if report.dns_resolvers.is_empty() {
        report
            .warnings
            .push(sc("Test DNS leak non riuscito: impossibile verificare i DNS", "DNS leak test failed: unable to verify DNS").into());
    }

    // WebRTC / IPv6
    if !report.webrtc_leak_detected {
        score += 10;
        report
            .positives
            .push(sc("WebRTC / UDP STUN sicuro: nessuna discrepanza tra IP HTTP e IP UDP.", "WebRTC / UDP STUN safe: no mismatch between the HTTP IP and the UDP IP.").into());
    } else {
        score -= 25;
        report.warnings.push(tr!(
            "WebRTC Leak rilevato! Il protocollo STUN UDP espone l'IP {}.",
            "WebRTC leak detected! The UDP STUN protocol exposes IP {}.",
            report.stun_ipv4.as_deref().unwrap_or("")
        ));
    }

    if report.ipv6_leak_detected {
        score -= 20;
        report.warnings.push(tr!(
            "IPv6 Leak rilevato! Il traffico IPv6 ({}) esce al di fuori del tunnel VPN.",
            "IPv6 leak detected! IPv6 traffic ({}) goes outside the VPN tunnel.",
            report.ipv6.as_deref().unwrap_or("")
        ));
    } else if report.ipv6.is_none() {
        score += 5;
        report
            .positives
            .push(sc("IPv6 pubblico disabilitato o bloccato: nessun rischio di IPv6 Leak.", "Public IPv6 disabled or blocked: no IPv6 leak risk.").into());
    }

    // Blacklist: contano solo le liste in cui l'IP e` davvero "in lista" e la segnalazione di
    // Mullvad (None = "non in lista"; le liste non verificabili non contano).
    let mullvad_segnala = report.mullvad_blacklisted == Some(true);
    let in_lista_totale = report.blacklisted_count + usize::from(mullvad_segnala);
    if in_lista_totale == 0 {
        score += 5;
        let non_verificabili = report
            .blacklist_results
            .values()
            .filter(|v| v.stato == StatoDnsbl::NonVerificabile)
            .count();
        let verificate = report.blacklist_results.len() - non_verificabili;
        let mut testo = tr!("Indirizzo IP pulito (0 su {verificate} liste di blocco DNSBL verificate", "Clean IP address (0 of {verificate} DNSBL blocklists checked");
        if non_verificabili > 0 {
            testo.push_str(&tr!(", {non_verificabili} non verificabili", ", {non_verificabili} not verifiable"));
        }
        testo.push_str(").");
        report.positives.push(testo);
    } else {
        let listed: Vec<&str> = report
            .blacklist_results
            .iter()
            .filter(|(_, v)| v.stato == StatoDnsbl::InLista)
            .map(|(k, _)| k.as_str())
            .collect();
        if report.blacklisted_count == 0 {
            report.warnings.push(sc("IP segnalato da Mullvad come in blacklist", "IP flagged by Mullvad as blacklisted").into());
        } else {
            let mut testo = tr!(
                "L'indirizzo IP {} è segnalato in {} blacklist ({})",
                "IP address {} is listed on {} blacklists ({})",
                report.ipv4.as_deref().unwrap_or(""),
                report.blacklisted_count,
                listed.join(", ")
            );
            if mullvad_segnala {
                testo.push_str(sc(" e da Mullvad", " and by Mullvad"));
            }
            testo.push('.');
            report.warnings.push(testo);
        }
    }

    report.security_score = score.clamp(0, 100);

    // Stato sintetico (stile Mullvad Connection Check)
    let has_critical_leak =
        report.dns_leak_detected || report.webrtc_leak_detected || report.ipv6_leak_detected;
    let is_vpn_connected = has_tunnel || is_datacenter_or_vpn;

    if is_vpn_connected && is_datacenter_or_vpn && !has_critical_leak {
        if report.edns_leak_detected
            || in_lista_totale > 0
            || !has_tunnel
            || report.dns_resolvers.is_empty()
        {
            report.overall_status = OverallStatus::Warning;
            report.overall_title = sc("CONNESSIONE VPN ATTIVA (CON AVVISI)", "VPN CONNECTION ACTIVE (WITH WARNINGS)").into();
            report.overall_summary = tr!(
                "Il tuo traffico passa da {} ({}), ma ci sono {} avvisi non critici da verificare.",
                "Your traffic goes through {} ({}), but there are {} non-critical warnings to review.",
                report.isp,
                report.country,
                report.warnings.len()
            );
        } else {
            report.overall_status = OverallStatus::Protected;
            report.overall_title = sc("CONNESSIONE PROTETTA (VPN SICURA)", "CONNECTION PROTECTED (SECURE VPN)").into();
            report.overall_summary = tr!(
                "Traffico cifrato tramite VPN ({} - {}, {}). Nessun leak IP, DNS o WebRTC rilevato.",
                "Traffic encrypted through VPN ({} - {}, {}). No IP, DNS or WebRTC leaks detected.",
                report.isp, report.city, report.country
            );
        }
    } else if is_vpn_connected && has_critical_leak {
        report.overall_status = OverallStatus::Warning;
        report.overall_title = sc("ATTENZIONE: VPN ATTIVA MA CON LEAK RILEVATI!", "WARNING: VPN ACTIVE BUT LEAKS DETECTED!").into();
        report.overall_summary = sc("Sei connesso a una VPN, ma alcuni dati (DNS, WebRTC, IPv6 o IP reale) stanno uscendo fuori dal tunnel!", "You are connected to a VPN, but some data (DNS, WebRTC, IPv6 or your real IP) is leaving the tunnel!").into();
    } else if has_tunnel {
        // Tunnel locale attivo ma l'IP pubblico e` quello dell'operatore e non ci sono leak
        report.overall_status = OverallStatus::Warning;
        report.overall_title = sc("Tunnel attivo, traffico non protetto", "Tunnel active, traffic not protected").into();
        report.overall_summary = sc("È attiva un'interfaccia VPN/tunnel, ma il traffico Internet esce dall'IP del tuo operatore (split tunneling o VPN senza uscita, es. Tailscale).", "A VPN/tunnel interface is up, but Internet traffic leaves through your carrier's IP (split tunneling or a VPN with no exit node, e.g. Tailscale).").into();
    } else if report.vpn_sconosciuta {
        report.overall_status = OverallStatus::Warning;
        report.overall_title = sc("STATO VPN NON VERIFICABILE", "VPN STATUS CANNOT BE VERIFIED").into();
        report.overall_summary = sc("Il servizio di geolocalizzazione non ha fornito i dati su VPN/proxy: non si può stabilire se il traffico è protetto.", "The geolocation service gave no VPN/proxy data: it cannot be determined whether your traffic is protected.").into();
    } else {
        report.overall_status = OverallStatus::Unprotected;
        report.overall_title = sc("CONNESSIONE NON PROTETTA (NESSUNA VPN ATTIVA)", "CONNECTION NOT PROTECTED (NO VPN ACTIVE)").into();
        report.overall_summary = tr!(
            "Stai navigando con l'IP diretto del tuo operatore ({} - {}, {}). Nessun tunnel VPN attivo.",
            "You are browsing with your carrier's direct IP ({} - {}, {}). No VPN tunnel is active.",
            report.isp, report.city, report.country
        );
    }
}
