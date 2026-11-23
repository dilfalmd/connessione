//! STUN Binding via UDP (RFC 5389): scopre l'IP pubblico visto fuori dal proxy HTTP,
//! come farebbe WebRTC.

use std::net::{Ipv4Addr, ToSocketAddrs, UdpSocket};
use std::time::Duration;

const MAGIC_COOKIE: u32 = 0x2112_A442;
const TIMEOUT: Duration = Duration::from_secs(2);

pub const STUN_SERVERS: [(&str, u16); 3] = [
    ("stun.l.google.com", 19302),
    ("stun1.l.google.com", 19302),
    ("stun.cloudflare.com", 3478),
];

/// Binding Request: tipo 0x0001, lunghezza 0, magic cookie, transaction ID di 12 byte.
pub fn costruisci_richiesta(tid: &[u8; 12]) -> [u8; 20] {
    let mut req = [0u8; 20];
    req[0..2].copy_from_slice(&0x0001u16.to_be_bytes());
    req[2..4].copy_from_slice(&0u16.to_be_bytes());
    req[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    req[8..20].copy_from_slice(tid);
    req
}

/// Cerca XOR-MAPPED-ADDRESS (0x0020) o MAPPED-ADDRESS (0x0001) IPv4 nella risposta.
pub fn parse_risposta(data: &[u8]) -> Option<String> {
    let mut pos = 20usize;
    while pos + 4 <= data.len() {
        let atype = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let alen = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if (atype == 0x0020 || atype == 0x0001) && alen >= 8 {
            // Attributo troncato: il Python solleva e passa al server successivo.
            if pos + 12 > data.len() {
                return None;
            }
            let family = data[pos + 5];
            if family == 0x01 {
                let raw = u32::from_be_bytes([
                    data[pos + 8],
                    data[pos + 9],
                    data[pos + 10],
                    data[pos + 11],
                ]);
                let ip = if atype == 0x0020 { raw ^ MAGIC_COOKIE } else { raw };
                return Some(Ipv4Addr::from(ip).to_string());
            }
        }
        pos += 4 + alen + (4 - alen % 4) % 4;
    }
    None
}

fn query_server(host: &str, port: u16) -> Option<String> {
    let addr = (host, port)
        .to_socket_addrs()
        .ok()?
        .find(|a| a.is_ipv4())?;
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.set_read_timeout(Some(TIMEOUT)).ok()?;
    let mut tid = [0u8; 12];
    getrandom::fill(&mut tid).ok()?;
    sock.send_to(&costruisci_richiesta(&tid), addr).ok()?;
    let mut buf = [0u8; 1024];
    let (n, _) = sock.recv_from(&mut buf).ok()?;
    parse_risposta(&buf[..n])
}

/// Prova i server STUN in sequenza e restituisce il primo IP IPv4 ottenuto (bloccante).
pub fn query_stun_udp() -> Option<String> {
    STUN_SERVERS
        .iter()
        .find_map(|(host, port)| query_server(host, *port))
}

pub async fn query_stun_udp_async() -> Option<String> {
    tokio::task::spawn_blocking(query_stun_udp)
        .await
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn risposta(attr_type: u16, family: u8, addr: [u8; 4]) -> Vec<u8> {
        let mut v = vec![0x01, 0x01, 0x00, 0x0C, 0x21, 0x12, 0xA4, 0x42];
        v.extend_from_slice(&[7u8; 12]);
        v.extend_from_slice(&attr_type.to_be_bytes());
        v.extend_from_slice(&8u16.to_be_bytes());
        v.extend_from_slice(&[0, family, 0x12, 0x34]);
        v.extend_from_slice(&addr);
        v
    }

    #[test]
    fn richiesta_ben_formata() {
        let r = costruisci_richiesta(&[9u8; 12]);
        assert_eq!(&r[0..8], &[0, 1, 0, 0, 0x21, 0x12, 0xA4, 0x42]);
        assert_eq!(&r[8..], &[9u8; 12]);
    }

    #[test]
    fn xor_mapped_address() {
        // 203.0.113.7 XOR 0x2112A442
        let ip = Ipv4Addr::new(203, 0, 113, 7);
        let x = (u32::from(ip) ^ MAGIC_COOKIE).to_be_bytes();
        let data = risposta(0x0020, 1, x);
        assert_eq!(parse_risposta(&data).as_deref(), Some("203.0.113.7"));
    }

    #[test]
    fn mapped_address_senza_xor() {
        let data = risposta(0x0001, 1, [198, 51, 100, 4]);
        assert_eq!(parse_risposta(&data).as_deref(), Some("198.51.100.4"));
    }

    #[test]
    fn ignora_ipv6_e_attributi_con_padding() {
        // Prima SOFTWARE (0x8022) di 5 byte + 3 di padding, poi XOR-MAPPED-ADDRESS valida
        let mut data = vec![0x01, 0x01, 0x00, 0x00, 0x21, 0x12, 0xA4, 0x42];
        data.extend_from_slice(&[1u8; 12]);
        data.extend_from_slice(&[0x80, 0x22, 0x00, 0x05, b'a', b'b', b'c', b'd', b'e', 0, 0, 0]);
        let x = (u32::from(Ipv4Addr::new(8, 8, 4, 4)) ^ MAGIC_COOKIE).to_be_bytes();
        data.extend_from_slice(&[0x00, 0x20, 0x00, 0x08, 0, 1, 0, 0]);
        data.extend_from_slice(&x);
        assert_eq!(parse_risposta(&data).as_deref(), Some("8.8.4.4"));
        // Famiglia IPv6: nessun risultato
        assert_eq!(parse_risposta(&risposta(0x0020, 2, [1, 2, 3, 4])), None);
    }

    #[test]
    fn risposte_vuote_o_troncate() {
        assert_eq!(parse_risposta(&[]), None);
        assert_eq!(parse_risposta(&[0u8; 20]), None);
        let mut troncata = risposta(0x0020, 1, [1, 2, 3, 4]);
        troncata.truncate(28);
        assert_eq!(parse_risposta(&troncata), None);
    }
}
