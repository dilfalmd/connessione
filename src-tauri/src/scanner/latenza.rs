//! Latenza TCP verso endpoint affidabili (1.1.1.1, 8.8.8.8, OpenDNS), media dei primi 2 campioni.

use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::timeout;

const TARGET: [(&str, u16); 3] = [("1.1.1.1", 443), ("8.8.8.8", 443), ("208.67.222.222", 443)];
const TIMEOUT: Duration = Duration::from_millis(1800);
const CAMPIONI_MAX: usize = 2;

/// Media dei campioni arrotondata a 1 decimale; `None` se non ci sono campioni.
pub fn media_arrotondata(campioni: &[f64]) -> Option<f64> {
    if campioni.is_empty() {
        return None;
    }
    let media = campioni.iter().sum::<f64>() / campioni.len() as f64;
    Some((media * 10.0).round() / 10.0)
}

pub async fn misura_latenza() -> Option<f64> {
    let mut campioni = Vec::new();
    for (host, port) in TARGET {
        let t0 = Instant::now();
        if let Ok(Ok(_)) = timeout(TIMEOUT, TcpStream::connect((host, port))).await {
            campioni.push(t0.elapsed().as_secs_f64() * 1000.0);
            if campioni.len() >= CAMPIONI_MAX {
                break;
            }
        }
    }
    media_arrotondata(&campioni)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn media_e_arrotondamento() {
        assert_eq!(media_arrotondata(&[]), None);
        assert_eq!(media_arrotondata(&[12.34]), Some(12.3));
        assert_eq!(media_arrotondata(&[10.0, 21.0]), Some(15.5));
    }
}
