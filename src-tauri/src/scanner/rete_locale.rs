//! Ispezione della rete locale: interfacce attive, riconoscimento dei tunnel VPN, server DNS
//! di sistema e controllo della rotta di default (kill switch). Usa le API IP Helper di
//! Windows (GetAdaptersAddresses / GetIpForwardTable2), senza processi esterni.

use super::report::{IPV6_PLACEHOLDER, TunnelInterface};

pub const TUNNEL_IFACE_KEYWORDS: &[&str] = &[
    "mullvad", "wireguard", "wintun", "openvpn", "tap", "tun", "nordlynx",
    "proton", "protonvpn", "surfshark", "windscribe", "cyberghost", "tailscale", "zerotier",
    "warp", "vpn", "ppp", "ikev2", "utun",
];

/// Vista neutra di un adattatore di rete (riempita dalle API Windows, o da dati finti nei test).
#[derive(Debug, Clone, Default)]
pub struct AdapterInfo {
    pub if_index: u32,
    /// Nome "amichevole" (es. "Ethernet", "Wi-Fi")
    pub name: String,
    /// Descrizione del driver (es. "WireGuard Tunnel")
    pub description: String,
    pub oper_up: bool,
    pub mtu: u32,
    pub speed_mbps: u64,
    pub ipv4: Vec<String>,
    pub ipv6: Vec<String>,
    pub dns: Vec<String>,
}

/// Rotta IPv4 di default (0.0.0.0/0) o a meta` (0.0.0.0/1 = netmask 128.0.0.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteV4 {
    pub if_index: u32,
    pub prefix_len: u8,
}

#[derive(Debug, Clone, Default)]
pub struct LocalNetwork {
    pub tunnels: Vec<TunnelInterface>,
    pub all_up: Vec<TunnelInterface>,
    pub dns_servers: Vec<String>,
    pub default_route_via_tunnel: bool,
    pub killswitch_or_strict_route: bool,
}

/// Interfacce virtuali di Windows che non sono VPN (tunnel IPv6 automatici, loopback, debug).
const INTERFACCE_ESCLUSE: &[&str] = &[
    "teredo",
    "isatap",
    "6to4",
    "loopback pseudo",
    "microsoft kernel debug",
];

fn e_interfaccia_esclusa(a: &AdapterInfo) -> bool {
    let nome = a.name.to_lowercase();
    let descrizione = a.description.to_lowercase();
    INTERFACCE_ESCLUSE
        .iter()
        .any(|x| nome.contains(x) || descrizione.contains(x))
}

/// Una parola chiave di tunnel combacia con una parola intera del testo, oppure con l'inizio di
/// una parola seguito solo da cifre o dalla fine ("tun0", "TAP-Windows"), mai dentro altre
/// parole ("Tunneling", "Intel(R) Ethernet", "PPPoE").
fn contiene_keyword_tunnel(testo: &str) -> bool {
    testo
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .any(|token| {
            TUNNEL_IFACE_KEYWORDS.iter().any(|k| {
                token
                    .strip_prefix(k)
                    .is_some_and(|resto| resto.chars().all(|c| c.is_ascii_digit()))
            })
        })
}

/// Logica pura: classifica le interfacce e valuta DNS e rotte.
pub fn analizza(adapters: &[AdapterInfo], routes: &[RouteV4]) -> LocalNetwork {
    let mut all_up: Vec<TunnelInterface> = Vec::new();
    let mut all_up_idx: Vec<u32> = Vec::new(); // if_index parallelo ad all_up
    let mut is_tunnel: Vec<bool> = Vec::new();
    let mut da_descrizione: Vec<bool> = Vec::new();

    for a in adapters {
        if !a.oper_up || e_interfaccia_esclusa(a) {
            continue;
        }
        let name_low = a.name.to_lowercase();
        if name_low.contains("loopback") || name_low == "lo" {
            continue;
        }

        let mut ipv4_str = String::new();
        for addr in &a.ipv4 {
            if !addr.starts_with("127.") && !addr.starts_with("169.254.") {
                ipv4_str = addr.clone();
            }
        }
        let mut ipv6_str = String::new();
        for addr in &a.ipv6 {
            if addr.starts_with("::1") {
                continue;
            }
            let pulito = addr.split('%').next().unwrap_or("");
            if !pulito.to_lowercase().starts_with("fe80:") {
                ipv6_str = pulito.to_string();
            }
        }
        if ipv4_str.is_empty() && ipv6_str.is_empty() {
            continue;
        }

        let mut tun = contiene_keyword_tunnel(&name_low);
        // Molti tunnel WireGuard / OpenVPN usano subnet 10.x o 100.x o MTU diversa da 1500
        if !tun
            && !ipv4_str.is_empty()
            && (ipv4_str.starts_with("10.") || ipv4_str.starts_with("100."))
            && ((0 < a.mtu && a.mtu < 1500) || a.mtu == 65535)
        {
            tun = true;
        }

        all_up.push(TunnelInterface {
            name: a.name.clone(),
            ipv4: if ipv4_str.is_empty() { "N/D".into() } else { ipv4_str },
            ipv6: if ipv6_str.is_empty() { IPV6_PLACEHOLDER.into() } else { ipv6_str },
            mtu: a.mtu,
            is_up: true,
            speed_mbps: a.speed_mbps,
            is_tunnel: tun,
        });
        all_up_idx.push(a.if_index);
        is_tunnel.push(tun);
        da_descrizione.push(false);
    }

    // Tunnel con nome "normale" ma descrizione del driver da tunnel (WireGuard, TAP-Windows, Wintun...)
    for a in adapters {
        if e_interfaccia_esclusa(a) || !contiene_keyword_tunnel(&a.description) {
            continue;
        }
        for addr in &a.ipv4 {
            for (i, item) in all_up.iter_mut().enumerate() {
                if item.ipv4 == *addr && !is_tunnel[i] {
                    item.is_tunnel = true;
                    is_tunnel[i] = true;
                    da_descrizione[i] = true;
                }
            }
        }
    }

    // Elenco dei tunnel nell'ordine di scoperta del Python: prima quelli per nome/regola,
    // poi quelli aggiunti dalla descrizione del driver.
    let mut tunnels: Vec<TunnelInterface> = Vec::new();
    let mut tunnel_idx: Vec<u32> = Vec::new();
    for fase_descrizione in [false, true] {
        for i in 0..all_up.len() {
            if is_tunnel[i] && da_descrizione[i] == fase_descrizione {
                tunnels.push(all_up[i].clone());
                tunnel_idx.push(all_up_idx[i]);
            }
        }
    }

    // Server DNS configurati sulle interfacce attive (esclusi i placeholder fec0:)
    let mut dns_servers: Vec<String> = Vec::new();
    for a in adapters.iter().filter(|a| a.oper_up && !e_interfaccia_esclusa(a)) {
        for d in &a.dns {
            if !d.is_empty()
                && (d.contains('.') || d.contains(':'))
                && !d.starts_with("fec0")
                && !dns_servers.contains(d)
            {
                dns_servers.push(d.clone());
            }
        }
    }

    // La rotta di default passa per un tunnel?
    let mut default_route_via_tunnel = false;
    let mut killswitch = false;
    if !tunnels.is_empty() {
        let tunnel_if: Vec<u32> = tunnels
            .iter()
            .zip(tunnel_idx.iter())
            .filter(|(t, _)| t.ipv4 != "N/D")
            .map(|(_, idx)| *idx)
            .collect();
        let mut zero_routes = 0;
        for r in routes {
            if r.prefix_len > 1 {
                continue;
            }
            zero_routes += 1;
            let via_tunnel = tunnel_if.contains(&r.if_index);
            if via_tunnel {
                default_route_via_tunnel = true;
            }
            if r.prefix_len == 1 && via_tunnel {
                killswitch = true;
            }
        }
        if default_route_via_tunnel && zero_routes == 1 {
            killswitch = true;
        }
    }

    LocalNetwork {
        tunnels,
        all_up,
        dns_servers,
        default_route_via_tunnel,
        killswitch_or_strict_route: killswitch,
    }
}

/// Raccoglie dati dal sistema e li analizza (bloccante).
pub fn ispeziona_rete_locale() -> LocalNetwork {
    let adapters = windows_api::adattatori();
    let routes = windows_api::rotte_default_v4();
    analizza(&adapters, &routes)
}

pub async fn ispeziona_rete_locale_async() -> LocalNetwork {
    tokio::task::spawn_blocking(ispeziona_rete_locale)
        .await
        .unwrap_or_default()
}

mod windows_api {
    use std::ffi::c_void;
    use std::net::{Ipv4Addr, Ipv6Addr};

    use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, NO_ERROR};
    use windows::Win32::NetworkManagement::IpHelper::{
        FreeMibTable, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses,
        GetIpForwardTable2, IP_ADAPTER_ADDRESSES_LH, MIB_IPFORWARD_TABLE2,
    };
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
    use windows::Win32::Networking::WinSock::{
        AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR, SOCKADDR_IN, SOCKADDR_IN6, SOCKET_ADDRESS,
    };

    use super::{AdapterInfo, RouteV4};

    /// Converte un SOCKET_ADDRESS in (IPv4, IPv6) testuali.
    unsafe fn indirizzo(sa: &SOCKET_ADDRESS) -> Option<(bool, String)> {
        if sa.lpSockaddr.is_null() {
            return None;
        }
        let base: *const SOCKADDR = sa.lpSockaddr;
        let family = unsafe { (*base).sa_family };
        if family == AF_INET {
            let s = unsafe { &*(base as *const SOCKADDR_IN) };
            let bytes = unsafe { s.sin_addr.S_un.S_addr }.to_ne_bytes();
            Some((true, Ipv4Addr::from(bytes).to_string()))
        } else if family == AF_INET6 {
            let s = unsafe { &*(base as *const SOCKADDR_IN6) };
            let bytes = unsafe { s.sin6_addr.u.Byte };
            let ip = Ipv6Addr::from(bytes);
            let mut testo = ip.to_string();
            let scope = unsafe { s.Anonymous.sin6_scope_id };
            if scope != 0 && (ip.segments()[0] & 0xffc0) == 0xfe80 {
                testo = format!("{testo}%{scope}");
            }
            Some((false, testo))
        } else {
            None
        }
    }

    pub fn adattatori() -> Vec<AdapterInfo> {
        let mut out = Vec::new();
        let flags = GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
        // Buffer allineato a 8 byte
        let mut buf: Vec<u64> = vec![0; 16 * 1024 / 8];
        let mut size: u32 = (buf.len() * 8) as u32;
        for _ in 0..5 {
            let ret = unsafe {
                GetAdaptersAddresses(
                    AF_UNSPEC.0 as u32,
                    flags,
                    None,
                    Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                    &mut size,
                )
            };
            if ret == ERROR_BUFFER_OVERFLOW.0 {
                buf = vec![0; (size as usize).div_ceil(8) + 1];
                size = (buf.len() * 8) as u32;
                continue;
            }
            if ret != NO_ERROR.0 {
                return out;
            }
            let mut cur = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
            while !cur.is_null() {
                let a = unsafe { &*cur };
                let mut info = AdapterInfo {
                    if_index: unsafe { a.Anonymous1.Anonymous.IfIndex },
                    name: unsafe { a.FriendlyName.to_string() }.unwrap_or_default(),
                    description: unsafe { a.Description.to_string() }.unwrap_or_default(),
                    oper_up: a.OperStatus == IfOperStatusUp,
                    mtu: a.Mtu,
                    speed_mbps: if a.TransmitLinkSpeed == u64::MAX {
                        0
                    } else {
                        a.TransmitLinkSpeed / 1_000_000
                    },
                    ..Default::default()
                };
                let mut u = a.FirstUnicastAddress;
                while !u.is_null() {
                    let ua = unsafe { &*u };
                    if let Some((v4, testo)) = unsafe { indirizzo(&ua.Address) } {
                        if v4 {
                            info.ipv4.push(testo);
                        } else {
                            info.ipv6.push(testo);
                        }
                    }
                    u = ua.Next;
                }
                let mut d = a.FirstDnsServerAddress;
                while !d.is_null() {
                    let da = unsafe { &*d };
                    if let Some((_, testo)) = unsafe { indirizzo(&da.Address) } {
                        info.dns.push(testo);
                    }
                    d = da.Next;
                }
                out.push(info);
                cur = a.Next;
            }
            return out;
        }
        out
    }

    /// Rotte IPv4 con destinazione 0.0.0.0 e prefisso /0 o /1.
    pub fn rotte_default_v4() -> Vec<RouteV4> {
        let mut out = Vec::new();
        let mut table: *mut MIB_IPFORWARD_TABLE2 = std::ptr::null_mut();
        let ret = unsafe { GetIpForwardTable2(AF_INET, &mut table) };
        if ret != NO_ERROR || table.is_null() {
            return out;
        }
        unsafe {
            let n = (*table).NumEntries as usize;
            let rows = std::slice::from_raw_parts((*table).Table.as_ptr(), n);
            for row in rows {
                let prefix_len = row.DestinationPrefix.PrefixLength;
                let dest = row.DestinationPrefix.Prefix.Ipv4.sin_addr.S_un.S_addr;
                if dest == 0 && prefix_len <= 1 {
                    out.push(RouteV4 {
                        if_index: row.InterfaceIndex,
                        prefix_len,
                    });
                }
            }
            FreeMibTable(table as *const c_void);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(name: &str, desc: &str, idx: u32, ip: &str, mtu: u32) -> AdapterInfo {
        AdapterInfo {
            if_index: idx,
            name: name.into(),
            description: desc.into(),
            oper_up: true,
            mtu,
            speed_mbps: 1000,
            ipv4: vec![ip.into()],
            ipv6: vec!["fe80::1%5".into()],
            dns: vec![],
        }
    }

    #[test]
    fn ethernet_semplice_non_e_tunnel() {
        let a = adapter("Ethernet", "Realtek PCIe GbE", 3, "192.168.1.10", 1500);
        let r = analizza(&[a], &[RouteV4 { if_index: 3, prefix_len: 0 }]);
        assert_eq!(r.all_up.len(), 1);
        assert!(r.tunnels.is_empty());
        assert_eq!(r.all_up[0].ipv6, IPV6_PLACEHOLDER);
        assert!(!r.default_route_via_tunnel);
    }

    #[test]
    fn tunnel_per_nome_e_rotte_killswitch() {
        let eth = adapter("Ethernet", "Realtek", 3, "192.168.1.10", 1500);
        let wg = adapter("Mullvad-SE", "WireGuard Tunnel", 9, "10.64.0.2", 1380);
        let routes = [
            RouteV4 { if_index: 9, prefix_len: 1 },
            RouteV4 { if_index: 9, prefix_len: 1 },
            RouteV4 { if_index: 3, prefix_len: 0 },
        ];
        let r = analizza(&[eth, wg], &routes);
        assert_eq!(r.tunnels.len(), 1);
        assert_eq!(r.tunnels[0].name, "Mullvad-SE");
        assert!(r.default_route_via_tunnel);
        assert!(r.killswitch_or_strict_route);
    }

    #[test]
    fn rotta_unica_nel_tunnel_vale_come_strict() {
        let wg = adapter("wg0", "WireGuard", 9, "10.0.0.2", 1420);
        let r = analizza(&[wg], &[RouteV4 { if_index: 9, prefix_len: 0 }]);
        assert!(r.default_route_via_tunnel && r.killswitch_or_strict_route);
    }

    #[test]
    fn rotta_default_fuori_dal_tunnel() {
        let eth = adapter("Ethernet", "Realtek", 3, "192.168.1.10", 1500);
        let wg = adapter("wg0", "WireGuard", 9, "10.0.0.2", 1420);
        let r = analizza(&[eth, wg], &[RouteV4 { if_index: 3, prefix_len: 0 }]);
        assert_eq!(r.tunnels.len(), 1);
        assert!(!r.default_route_via_tunnel && !r.killswitch_or_strict_route);
    }

    #[test]
    fn tunnel_riconosciuto_dalla_descrizione_del_driver() {
        let a = adapter("Ethernet 2", "TAP-Windows Adapter V9", 7, "172.16.0.5", 1500);
        let r = analizza(&[a], &[]);
        assert_eq!(r.tunnels.len(), 1);
        assert!(r.all_up[0].is_tunnel);
    }

    #[test]
    fn regola_subnet_10_con_mtu_ridotta() {
        let a = adapter("Rete interna", "Adattatore generico", 4, "10.8.0.6", 1400);
        assert_eq!(analizza(&[a], &[]).tunnels.len(), 1);
        let b = adapter("Rete interna", "Adattatore generico", 4, "10.8.0.6", 1500);
        assert!(analizza(&[b], &[]).tunnels.is_empty());
    }

    #[test]
    fn salta_down_loopback_apipa_e_senza_indirizzi() {
        let mut down = adapter("Wi-Fi", "x", 1, "192.168.0.2", 1500);
        down.oper_up = false;
        let lo = adapter("Loopback Pseudo-Interface 1", "x", 2, "127.0.0.1", 65535);
        let apipa = AdapterInfo { ipv6: vec![], ..adapter("Ethernet 3", "x", 3, "169.254.1.1", 1500) };
        let r = analizza(&[down, lo, apipa], &[]);
        assert!(r.all_up.is_empty());
    }

    #[test]
    fn dns_di_sistema_senza_duplicati_e_senza_fec0() {
        let mut a = adapter("Ethernet", "x", 3, "192.168.1.10", 1500);
        a.dns = vec![
            "192.168.1.1".into(),
            "fec0:0:0:ffff::1%1".into(),
            "8.8.8.8".into(),
            "192.168.1.1".into(),
        ];
        let mut b = adapter("Wi-Fi", "x", 4, "192.168.1.11", 1500);
        b.dns = vec!["8.8.8.8".into(), "2001:4860:4860::8888".into()];
        let r = analizza(&[a, b], &[]);
        assert_eq!(
            r.dns_servers,
            vec!["192.168.1.1", "8.8.8.8", "2001:4860:4860::8888"]
        );
    }

    #[test]
    fn interfacce_virtuali_di_windows_sono_escluse() {
        let mut teredo = adapter("Teredo Tunneling Pseudo-Interface", "Microsoft Teredo Tunneling Adapter", 11, "10.9.9.9", 1280);
        teredo.ipv6 = vec!["2001:0:1:2::3".into()];
        let isatap = adapter("isatap.lan", "Microsoft ISATAP Adapter", 12, "10.8.8.8", 1280);
        let seitoquattro = adapter("6to4 Adapter", "Microsoft 6to4 Adapter", 13, "10.7.7.7", 1280);
        let pseudo = adapter("Ethernet 9", "Loopback Pseudo-Interface 1", 14, "10.6.6.6", 1280);
        let debug = adapter("Ethernet 10", "Microsoft Kernel Debug Network Adapter", 15, "10.5.5.5", 1280);
        let mut con_dns = adapter("Teredo Tunneling Pseudo-Interface", "Teredo", 16, "10.4.4.4", 1280);
        con_dns.dns = vec!["8.8.4.4".into()];
        let r = analizza(&[teredo, isatap, seitoquattro, pseudo, debug, con_dns], &[]);
        assert!(r.all_up.is_empty(), "nessuna interfaccia virtuale conta come attiva: {:?}", r.all_up);
        assert!(r.tunnels.is_empty());
        assert!(r.dns_servers.is_empty());
    }

    #[test]
    fn parole_chiave_dei_tunnel_per_parola_intera() {
        for si in ["Wintun Userspace Tunnel", "TAP-Windows Adapter V9", "WireGuard Tunnel", "tun0", "TUN1", "ppp0", "OpenVPN Data Channel Offload", "ProtonVPN", "Tailscale", "NordLynx", "vpn"] {
            assert!(contiene_keyword_tunnel(si), "{si} dovrebbe combaciare");
        }
        for no in ["Microsoft Teredo Tunneling Adapter", "Intel(R) Ethernet Connection", "Realtek PCIe GbE", "WAN Miniport (PPPOE)", "Hyper-V Virtual Switch Extension", "Bluetooth Device (Personal Area Network)", "Microsoft Wi-Fi Direct Virtual Adapter", "Atapi Adapter", "Mastune Network"] {
            assert!(!contiene_keyword_tunnel(no), "{no} non deve combaciare");
        }
    }

    #[test]
    fn wintun_tap_e_wireguard_restano_tunnel() {
        let wintun = adapter("Connessione 1", "Wintun Userspace Tunnel", 21, "172.16.5.2", 1500);
        let tap = adapter("Ethernet 3", "TAP-Windows Adapter V9", 22, "172.16.6.2", 1500);
        let wg = adapter("Rete lavoro", "WireGuard Tunnel", 23, "172.16.7.2", 1500);
        let r = analizza(&[wintun, tap, wg], &[]);
        assert_eq!(r.tunnels.len(), 3);
    }

    #[test]
    fn ipv6_globale_viene_riportato() {
        let mut a = adapter("wg0", "WireGuard", 9, "10.0.0.2", 1420);
        a.ipv6 = vec!["fe80::1%9".into(), "2001:db8::5".into()];
        let r = analizza(&[a], &[]);
        assert_eq!(r.tunnels[0].ipv6, "2001:db8::5");
    }
}
