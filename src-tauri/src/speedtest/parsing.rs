//! Interpretazione delle righe JSON (`--format=jsonl`) e dell'elenco server (`-L --format=json`)
//! dello Speedtest CLI di Ookla. Funzioni pure, testate con fixture in `tests/fixtures/`.

use serde_json::Value;

use super::tipi::{FaseSpeedtest, MediaSpeedtest, ServerSpeedtest};
use crate::archivio::MisuraVelocita;
use crate::testi::{sc, tr};

/// Risultato finale di una misura, come riportato dalla CLI (valori già convertiti in Mbps).
#[derive(Debug, Clone, PartialEq)]
pub struct RisultatoCli {
    pub ping_ms: f64,
    pub jitter_ms: Option<f64>,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub packet_loss: Option<f64>,
    pub isp: Option<String>,
    pub ip_esterno: Option<String>,
    pub server_id: Option<u32>,
    pub server_nome: Option<String>,
    pub server_luogo: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EventoCli {
    Progresso {
        fase: FaseSpeedtest,
        avanzamento: f64,
        valore: f64,
    },
    Risultato(RisultatoCli),
    /// Riga `log` con livello error, oppure risultato incompleto.
    Errore(String),
    /// testStart, log non critici, tipi sconosciuti.
    Ignorato,
}

/// byte/s -> Mbps
pub fn bandwidth_a_mbps(byte_al_secondo: f64) -> f64 {
    byte_al_secondo * 8.0 / 1e6
}

fn num(v: &Value, percorso: &[&str]) -> Option<f64> {
    let mut cur = v;
    for chiave in percorso {
        cur = cur.get(chiave)?;
    }
    cur.as_f64()
}

fn testo(v: &Value, percorso: &[&str]) -> Option<String> {
    let mut cur = v;
    for chiave in percorso {
        cur = cur.get(chiave)?;
    }
    cur.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Interpreta una riga di output. `None` se la riga non è JSON (es. testo libero).
pub fn parse_riga(riga: &str) -> Option<EventoCli> {
    let riga = riga.trim();
    if riga.is_empty() {
        return None;
    }
    let v: Value = serde_json::from_str(riga).ok()?;
    let tipo = v.get("type").and_then(Value::as_str).unwrap_or("");
    Some(match tipo {
        "ping" => EventoCli::Progresso {
            fase: FaseSpeedtest::Ping,
            avanzamento: num(&v, &["ping", "progress"]).unwrap_or(0.0).clamp(0.0, 1.0),
            valore: num(&v, &["ping", "latency"]).unwrap_or(0.0),
        },
        "download" | "upload" => {
            let (fase, chiave) = if tipo == "download" {
                (FaseSpeedtest::Download, "download")
            } else {
                (FaseSpeedtest::Upload, "upload")
            };
            EventoCli::Progresso {
                fase,
                avanzamento: num(&v, &[chiave, "progress"]).unwrap_or(0.0).clamp(0.0, 1.0),
                valore: bandwidth_a_mbps(num(&v, &[chiave, "bandwidth"]).unwrap_or(0.0)),
            }
        }
        "result" => match (
            num(&v, &["ping", "latency"]),
            num(&v, &["download", "bandwidth"]),
            num(&v, &["upload", "bandwidth"]),
        ) {
            (Some(ping), Some(down), Some(up)) => EventoCli::Risultato(RisultatoCli {
                ping_ms: ping,
                jitter_ms: num(&v, &["ping", "jitter"]),
                download_mbps: bandwidth_a_mbps(down),
                upload_mbps: bandwidth_a_mbps(up),
                packet_loss: num(&v, &["packetLoss"]),
                isp: testo(&v, &["isp"]),
                ip_esterno: testo(&v, &["interface", "externalIp"]),
                server_id: num(&v, &["server", "id"]).map(|n| n as u32),
                server_nome: testo(&v, &["server", "name"]),
                server_luogo: testo(&v, &["server", "location"]),
                url: testo(&v, &["result", "url"]),
            }),
            _ => EventoCli::Errore(sc("Risultato incompleto restituito dalla CLI", "Incomplete result returned by the CLI").into()),
        },
        "log" => {
            if v.get("level").and_then(Value::as_str) == Some("error") {
                EventoCli::Errore(
                    testo(&v, &["message"]).unwrap_or_else(|| sc("Errore sconosciuto", "Unknown error").into()),
                )
            } else {
                EventoCli::Ignorato
            }
        }
        _ => EventoCli::Ignorato,
    })
}

/// Elenco server da `speedtest -L --format=json`.
pub fn parse_elenco_server(json: &str) -> Result<Vec<ServerSpeedtest>, String> {
    let v: Value = serde_json::from_str(json.trim()).map_err(|e| tr!("JSON non valido: {e}", "Invalid JSON: {e}"))?;
    let servers = v
        .get("servers")
        .and_then(Value::as_array)
        .ok_or(sc("Elenco server assente nella risposta", "Server list missing from the response"))?;
    Ok(servers
        .iter()
        .filter_map(|s| {
            Some(ServerSpeedtest {
                id: s.get("id")?.as_u64()? as u32,
                nome: testo(s, &["name"]).unwrap_or_default(),
                luogo: testo(s, &["location"]).unwrap_or_default(),
                paese: testo(s, &["country"]).unwrap_or_default(),
                host: testo(s, &["host"]).unwrap_or_default(),
            })
        })
        .collect())
}

fn media_opzionale(valori: impl Iterator<Item = Option<f64>>) -> Option<f64> {
    let presenti: Vec<f64> = valori.flatten().collect();
    if presenti.is_empty() {
        None
    } else {
        Some(presenti.iter().sum::<f64>() / presenti.len() as f64)
    }
}

/// Media delle misure di una sessione. Il packet loss (e il jitter) si mediano sulle sole misure
/// che li riportano; se nessuna li ha restano `None`. `None` se non ci sono misure.
pub fn media_misure(misure: &[MisuraVelocita]) -> Option<MediaSpeedtest> {
    if misure.is_empty() {
        return None;
    }
    let n = misure.len() as f64;
    Some(MediaSpeedtest {
        ping_ms: misure.iter().map(|m| m.ping_ms).sum::<f64>() / n,
        jitter_ms: media_opzionale(misure.iter().map(|m| m.jitter_ms)),
        download_mbps: misure.iter().map(|m| m.download_mbps).sum::<f64>() / n,
        upload_mbps: misure.iter().map(|m| m.upload_mbps).sum::<f64>() / n,
        packet_loss: media_opzionale(misure.iter().map(|m| m.packet_loss)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSONL: &str = include_str!("../../tests/fixtures/speedtest_jsonl_completo.jsonl");
    const SENZA_LOSS: &str =
        include_str!("../../tests/fixtures/speedtest_risultato_senza_packetloss.jsonl");
    const ERRORE: &str = include_str!("../../tests/fixtures/speedtest_errore.jsonl");
    const ELENCO: &str = include_str!("../../tests/fixtures/server_elenco.json");

    fn eventi(testo: &str) -> Vec<EventoCli> {
        testo.lines().filter_map(parse_riga).collect()
    }

    fn misura(ping: f64, jitter: Option<f64>, down: f64, up: f64, loss: Option<f64>) -> MisuraVelocita {
        MisuraVelocita {
            id: None,
            timestamp: "t".into(),
            sessione: "s".into(),
            isp: None,
            ip_esterno: None,
            server_id: None,
            server_nome: None,
            server_luogo: None,
            ping_ms: ping,
            jitter_ms: jitter,
            download_mbps: down,
            upload_mbps: up,
            packet_loss: loss,
            url: None,
            vpn_attiva: None,
            controllo_id: None,
            origine: "app".into(),
        }
    }

    #[test]
    fn conversione_bandwidth() {
        assert_eq!(bandwidth_a_mbps(12_500_000.0), 100.0);
        assert_eq!(bandwidth_a_mbps(0.0), 0.0);
    }

    #[test]
    fn righe_di_avanzamento() {
        let ev = eventi(JSONL);
        assert_eq!(ev.len(), 9, "testStart + 2 ping + 3 download + 2 upload + result");
        assert_eq!(ev[0], EventoCli::Ignorato, "testStart");
        assert_eq!(
            ev[1],
            EventoCli::Progresso { fase: FaseSpeedtest::Ping, avanzamento: 0.5, valore: 21.334 }
        );
        assert_eq!(
            ev[2],
            EventoCli::Progresso { fase: FaseSpeedtest::Ping, avanzamento: 1.0, valore: 22.101 }
        );
        assert_eq!(
            ev[4],
            EventoCli::Progresso { fase: FaseSpeedtest::Download, avanzamento: 0.55, valore: 100.0 }
        );
        assert_eq!(
            ev[7],
            EventoCli::Progresso { fase: FaseSpeedtest::Upload, avanzamento: 1.0, valore: 12.0 }
        );
    }

    #[test]
    fn riga_di_risultato_completa() {
        let ev = eventi(JSONL);
        let EventoCli::Risultato(r) = ev.last().unwrap() else {
            panic!("ultimo evento non è un risultato");
        };
        assert_eq!(r.ping_ms, 22.101);
        assert_eq!(r.jitter_ms, Some(1.204));
        assert_eq!(r.download_mbps, 104.0);
        assert_eq!(r.upload_mbps, 12.0);
        assert_eq!(r.packet_loss, Some(1.5));
        assert_eq!(r.isp.as_deref(), Some("Provider Esempio"));
        assert_eq!(r.ip_esterno.as_deref(), Some("203.0.113.7"));
        assert_eq!(r.server_id, Some(22857));
        assert_eq!(r.server_nome.as_deref(), Some("Server Esempio"));
        assert_eq!(r.server_luogo.as_deref(), Some("Milano"));
        assert_eq!(
            r.url.as_deref(),
            Some("https://www.speedtest.net/result/c/11111111-2222-3333-4444-555555555555")
        );
    }

    #[test]
    fn risultato_senza_packet_loss() {
        let ev = eventi(SENZA_LOSS);
        let EventoCli::Risultato(r) = &ev[0] else { panic!() };
        assert_eq!(r.packet_loss, None);
        assert_eq!(r.download_mbps, 200.0);
        assert_eq!(r.upload_mbps, 20.0);
    }

    #[test]
    fn errore_dalla_cli_e_righe_non_json() {
        assert_eq!(
            eventi(ERRORE),
            vec![EventoCli::Errore("Configuration - Couldn't resolve host name (HostNotFoundException)".into())]
        );
        assert_eq!(parse_riga("testo libero"), None);
        assert_eq!(parse_riga("   "), None);
        assert_eq!(
            parse_riga(r#"{"type":"log","message":"info","level":"info"}"#),
            Some(EventoCli::Ignorato)
        );
        assert_eq!(parse_riga(r#"{"type":"nuovo"}"#), Some(EventoCli::Ignorato));
        assert!(matches!(
            parse_riga(r#"{"type":"result","ping":{"latency":1.0}}"#),
            Some(EventoCli::Errore(_))
        ));
    }

    #[test]
    fn avanzamento_limitato_a_0_1() {
        let Some(EventoCli::Progresso { avanzamento, .. }) =
            parse_riga(r#"{"type":"download","download":{"bandwidth":1,"progress":1.7}}"#)
        else {
            panic!()
        };
        assert_eq!(avanzamento, 1.0);
    }

    #[test]
    fn elenco_server_vero() {
        let s = parse_elenco_server(ELENCO).unwrap();
        assert_eq!(s.len(), 11);
        assert_eq!(s[0].id, 11842);
        assert_eq!(s[0].nome, "Server Esempio");
        assert_eq!(s[0].luogo, "Milano");
        assert_eq!(s[0].paese, "Italy");
        assert_eq!(s[0].host, "speedtest1.esempio.it");
        assert!(parse_elenco_server("{}").is_err());
        assert!(parse_elenco_server("non json").is_err());
    }

    #[test]
    fn media_con_packet_loss_mancante() {
        let m = media_misure(&[
            misura(10.0, Some(1.0), 100.0, 10.0, Some(2.0)),
            misura(20.0, Some(3.0), 200.0, 30.0, None),
            misura(30.0, None, 300.0, 20.0, Some(4.0)),
        ])
        .unwrap();
        assert_eq!(m.ping_ms, 20.0);
        assert_eq!(m.jitter_ms, Some(2.0));
        assert_eq!(m.download_mbps, 200.0);
        assert_eq!(m.upload_mbps, 20.0);
        assert_eq!(m.packet_loss, Some(3.0), "media di 2 e 4, la misura senza valore è esclusa");
    }

    #[test]
    fn media_senza_alcun_packet_loss_o_misure() {
        let m = media_misure(&[misura(10.0, None, 1.0, 1.0, None), misura(20.0, None, 3.0, 5.0, None)]).unwrap();
        assert_eq!(m.packet_loss, None);
        assert_eq!(m.jitter_ms, None);
        assert_eq!(m.download_mbps, 2.0);
        assert!(media_misure(&[]).is_none());
    }
}
