//! Dettagli IPv4 (ip-api.com, ipwho.is, API Mullvad) e verifica IPv6 pubblico.

use std::time::Duration;

use reqwest::Client;
use serde_json::Value;
use crate::testi::sconosciuto;

const URL_IP_API: &str = "http://ip-api.com/json/?fields=status,message,country,countryCode,regionName,city,timezone,lat,lon,isp,org,as,asname,reverse,proxy,hosting,query";
const URL_IPWHO: &str = "https://ipwho.is/";
const URL_MULLVAD: &str = "https://ipv4.am.i.mullvad.net/json";
const URL_IPV6: [&str; 2] = [
    "https://api6.ipify.org?format=json",
    "https://v6.ident.me/.json",
];

const TIMEOUT_IP_API: Duration = Duration::from_millis(8000);
/// Tentativi totali per ip-api.com (il primo può fallire su reti lente come un hotspot).
const TENTATIVI_IP_API: u32 = 2;
const TIMEOUT_IPWHO: Duration = Duration::from_millis(5000);
const TIMEOUT_MULLVAD: Duration = Duration::from_millis(3000);
const TIMEOUT_IPV6: Duration = Duration::from_millis(2500);

/// Dettagli sull'IPv4 pubblico. Il `Default` corrisponde al risultato vuoto del Python
/// (nessun servizio ha risposto): stringhe sconosciuto() dove il report le usa come default.
#[derive(Debug, Clone, PartialEq)]
pub struct Ipv4Details {
    pub ipv4: Option<String>,
    pub country: String,
    pub country_code: String,
    pub region: String,
    pub city: String,
    pub timezone: String,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub isp: String,
    pub org: String,
    pub asn: String,
    pub reverse: String,
    /// `None` = il servizio non lo dice (fallback ipwho.is): ignoto, né VPN né non-VPN.
    pub proxy: Option<bool>,
    pub hosting: Option<bool>,
}

impl Default for Ipv4Details {
    fn default() -> Self {
        Ipv4Details {
            ipv4: None,
            country: sconosciuto().into(),
            country_code: String::new(),
            region: String::new(),
            city: String::new(),
            timezone: String::new(),
            lat: None,
            lon: None,
            isp: sconosciuto().into(),
            org: sconosciuto().into(),
            asn: sconosciuto().into(),
            reverse: String::new(),
            proxy: None,
            hosting: None,
        }
    }
}

/// Esito del controllo opzionale sull'API di Mullvad.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MullvadInfo {
    pub is_mullvad_ip: bool,
    pub host: String,
    pub blacklisted: Option<bool>,
}

/// Stringa di un campo JSON, oppure `default` se manca o non e` testo/numero.
fn testo(v: &Value, chiave: &str, default: &str) -> String {
    match v.get(chiave) {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => default.to_string(),
    }
}

fn numero(v: &Value, chiave: &str) -> Option<f64> {
    v.get(chiave).and_then(Value::as_f64)
}

fn booleano(v: &Value, chiave: &str) -> bool {
    match v.get(chiave) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|x| x != 0.0),
        Some(Value::String(s)) => !s.is_empty(),
        _ => false,
    }
}

fn non_vuoto(s: String) -> Option<String> {
    if s.is_empty() { None } else { Some(s) }
}

/// Interpreta la risposta di ip-api.com. `None` se `status` non e` "success".
pub fn parse_ip_api(data: &Value) -> Option<Ipv4Details> {
    if data.get("status").and_then(Value::as_str) != Some("success") {
        return None;
    }
    let org = {
        let o = testo(data, "org", "");
        if o.is_empty() { testo(data, "asname", "") } else { o }
    };
    Some(Ipv4Details {
        ipv4: non_vuoto(testo(data, "query", "")),
        country: testo(data, "country", sconosciuto()),
        country_code: testo(data, "countryCode", ""),
        region: testo(data, "regionName", ""),
        city: testo(data, "city", ""),
        timezone: testo(data, "timezone", ""),
        lat: numero(data, "lat"),
        lon: numero(data, "lon"),
        isp: testo(data, "isp", sconosciuto()),
        org,
        asn: testo(data, "as", sconosciuto()),
        reverse: testo(data, "reverse", ""),
        proxy: Some(booleano(data, "proxy")),
        hosting: Some(booleano(data, "hosting")),
    })
}

/// Interpreta la risposta di ipwho.is (fallback). `None` se `success` non e` vero.
pub fn parse_ipwho(data: &Value) -> Option<Ipv4Details> {
    if !data.get("success").is_some_and(|v| matches!(v, Value::Bool(true))) {
        return None;
    }
    let vuoto = Value::Null;
    let conn = data.get("connection").unwrap_or(&vuoto);
    let tz = data.get("timezone").unwrap_or(&vuoto);
    let org = testo(conn, "org", "");
    let asn = format!("AS{} {}", testo(conn, "asn", ""), org).trim().to_string();
    Some(Ipv4Details {
        ipv4: non_vuoto(testo(data, "ip", "")),
        country: testo(data, "country", sconosciuto()),
        country_code: testo(data, "country_code", ""),
        region: testo(data, "region", ""),
        city: testo(data, "city", ""),
        timezone: testo(tz, "id", ""),
        lat: numero(data, "latitude"),
        lon: numero(data, "longitude"),
        isp: testo(conn, "isp", sconosciuto()),
        org,
        asn,
        reverse: testo(conn, "domain", ""),
        proxy: None,
        hosting: None,
    })
}

/// Interpreta la risposta di ipv4.am.i.mullvad.net/json.
pub fn parse_mullvad(data: &Value) -> MullvadInfo {
    let blacklisted = data
        .get("blacklisted")
        .and_then(Value::as_object)
        .and_then(|o| o.get("blacklisted"))
        .map(|v| match v {
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64().is_some_and(|x| x != 0.0),
            Value::Null => false,
            Value::String(s) => !s.is_empty(),
            _ => true,
        });
    MullvadInfo {
        is_mullvad_ip: booleano(data, "mullvad_exit_ip"),
        host: testo(data, "mullvad_exit_ip_hostname", ""),
        blacklisted,
    }
}

/// Estrae un IPv6 dalla risposta di api6.ipify.org / v6.ident.me (campi `ip` o `address`).
pub fn parse_ipv6(data: &Value) -> Option<String> {
    let ip = data
        .get("ip")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| data.get("address").and_then(Value::as_str));
    ip.filter(|s| s.contains(':')).map(str::to_string)
}

/// GET e decodifica JSON; `None` per qualsiasi errore o stato diverso da 200.
pub async fn get_json(client: &Client, url: &str, timeout: Duration) -> Option<Value> {
    let resp = client.get(url).timeout(timeout).send().await.ok()?;
    if resp.status().as_u16() != 200 {
        return None;
    }
    resp.json::<Value>().await.ok()
}

/// Come `get_json`, ma se il tentativo fallisce ne fa altri, fino a `tentativi` in tutto.
pub async fn get_json_con_ripetizione(
    client: &Client,
    url: &str,
    timeout: Duration,
    tentativi: u32,
) -> Option<Value> {
    for _ in 0..tentativi.max(1) {
        if let Some(v) = get_json(client, url, timeout).await {
            return Some(v);
        }
    }
    None
}

/// ip-api.com (8 s, un secondo tentativo se fallisce), poi ipwho.is se non ha dato un IP.
async fn dettagli_geo(client: &Client) -> Ipv4Details {
    let mut result = get_json_con_ripetizione(client, URL_IP_API, TIMEOUT_IP_API, TENTATIVI_IP_API)
        .await
        .and_then(|d| parse_ip_api(&d))
        .unwrap_or_default();
    if result.ipv4.is_none()
        && let Some(d) = get_json(client, URL_IPWHO, TIMEOUT_IPWHO)
            .await
            .and_then(|d| parse_ipwho(&d))
    {
        result = d;
    }
    result
}

async fn dettagli_mullvad(client: &Client) -> MullvadInfo {
    get_json(client, URL_MULLVAD, TIMEOUT_MULLVAD)
        .await
        .map(|d| parse_mullvad(&d))
        .unwrap_or_default()
}

/// Dettagli IPv4 + controllo Mullvad (eseguiti in parallelo: sono indipendenti).
pub async fn fetch_ipv4_details(client: &Client) -> (Ipv4Details, MullvadInfo) {
    tokio::join!(dettagli_geo(client), dettagli_mullvad(client))
}

/// Verifica se la connessione ha un IPv6 pubblico attivo (endpoint provati in sequenza).
pub async fn fetch_ipv6(client: &Client) -> Option<String> {
    for url in URL_IPV6 {
        if let Some(ip6) = get_json(client, url, TIMEOUT_IPV6)
            .await
            .and_then(|d| parse_ipv6(&d))
        {
            return Some(ip6);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn timeout_ip_api_8_secondi_con_due_tentativi() {
        assert_eq!(TIMEOUT_IP_API, Duration::from_secs(8));
        assert_eq!(TENTATIVI_IP_API, 2);
    }

    /// Server HTTP locale: a ogni connessione usa la prossima risposta (`None` = chiude subito).
    async fn server_finto(risposte: Vec<Option<&'static str>>) -> (String, tokio::task::JoinHandle<u32>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let h = tokio::spawn(async move {
            let mut connessioni = 0;
            for r in risposte {
                let (mut s, _) = listener.accept().await.unwrap();
                connessioni += 1;
                if let Some(corpo) = r {
                    let mut buf = [0u8; 1024];
                    let _ = s.read(&mut buf).await;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{corpo}",
                        corpo.len()
                    );
                    let _ = s.write_all(resp.as_bytes()).await;
                }
            }
            connessioni
        });
        (url, h)
    }

    #[tokio::test]
    async fn secondo_tentativo_se_il_primo_fallisce() {
        let (url, h) = server_finto(vec![None, Some("{\"status\":\"success\"}")]).await;
        let client = Client::new();
        let v = get_json_con_ripetizione(&client, &url, Duration::from_secs(5), 2).await;
        assert_eq!(v, Some(json!({"status": "success"})));
        assert_eq!(h.await.unwrap(), 2);
    }

    #[tokio::test]
    async fn un_solo_tentativo_non_ripete() {
        let (url, h) = server_finto(vec![None]).await;
        let client = Client::new();
        assert_eq!(get_json_con_ripetizione(&client, &url, Duration::from_secs(5), 1).await, None);
        assert_eq!(h.await.unwrap(), 1);
    }

    #[test]
    fn ip_api_successo() {
        let d = json!({
            "status": "success", "query": "203.0.113.7", "country": "Italy", "countryCode": "IT",
            "regionName": "Lazio", "city": "Rome", "timezone": "Europe/Rome", "lat": 41.9, "lon": 12.5,
            "isp": "Fastweb", "org": "", "asname": "FASTWEB-AS", "as": "AS12874 Fastweb S.p.A.",
            "reverse": "host.example", "proxy": false, "hosting": true
        });
        let r = parse_ip_api(&d).unwrap();
        assert_eq!(r.ipv4.as_deref(), Some("203.0.113.7"));
        assert_eq!(r.country_code, "IT");
        assert_eq!(r.org, "FASTWEB-AS", "org vuoto ricade su asname");
        assert_eq!(r.asn, "AS12874 Fastweb S.p.A.");
        assert_eq!((r.hosting, r.proxy), (Some(true), Some(false)));
        assert_eq!(r.lat, Some(41.9));
    }

    #[test]
    fn ip_api_fallimento() {
        assert!(parse_ip_api(&json!({"status": "fail", "message": "reserved range"})).is_none());
        assert!(parse_ip_api(&json!([1, 2])).is_none());
    }

    #[test]
    fn ipwho_successo_e_asn_formattato() {
        let d = json!({
            "success": true, "ip": "198.51.100.4", "country": "Sweden", "country_code": "SE",
            "region": "Stockholm", "city": "Stockholm", "latitude": 59.33, "longitude": 18.06,
            "connection": {"asn": 39351, "org": "31173 Services AB", "isp": "Mullvad", "domain": "31173.se"},
            "timezone": {"id": "Europe/Stockholm"}
        });
        let r = parse_ipwho(&d).unwrap();
        assert_eq!(r.asn, "AS39351 31173 Services AB");
        assert_eq!(r.timezone, "Europe/Stockholm");
        assert_eq!(r.reverse, "31173.se");
        assert_eq!((r.proxy, r.hosting), (None, None), "ipwho.is non dice nulla su proxy e hosting");
        assert!(parse_ipwho(&json!({"success": false})).is_none());
    }

    #[test]
    fn ipwho_senza_connection_da_asn_vuoto() {
        let r = parse_ipwho(&json!({"success": true, "ip": "1.2.3.4"})).unwrap();
        assert_eq!(r.asn, "AS");
        assert_eq!(r.isp, "Sconosciuto");
    }

    #[test]
    fn mullvad_ip_e_blacklist() {
        let d = json!({
            "mullvad_exit_ip": true, "mullvad_exit_ip_hostname": "se-sto-wg-001",
            "blacklisted": {"blacklisted": true, "results": []}
        });
        let m = parse_mullvad(&d);
        assert!(m.is_mullvad_ip);
        assert_eq!(m.host, "se-sto-wg-001");
        assert_eq!(m.blacklisted, Some(true));

        let m2 = parse_mullvad(&json!({"mullvad_exit_ip": false}));
        assert!(!m2.is_mullvad_ip);
        assert_eq!(m2.blacklisted, None);
    }

    #[test]
    fn ipv6_da_ip_o_address() {
        assert_eq!(
            parse_ipv6(&json!({"ip": "2001:db8::1"})).as_deref(),
            Some("2001:db8::1")
        );
        assert_eq!(
            parse_ipv6(&json!({"address": "2001:db8::2"})).as_deref(),
            Some("2001:db8::2")
        );
        assert_eq!(parse_ipv6(&json!({"ip": "203.0.113.7"})), None);
        assert_eq!(parse_ipv6(&json!({})), None);
    }
}
