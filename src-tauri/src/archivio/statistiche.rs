//! Statistiche sulle misure di velocità: confronto fra VPN accesa e spenta.

use serde::Serialize;

use super::Archivio;
use super::velocita::MisuraVelocita;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatMisura {
    pub media: Option<f64>,
    pub mediana: Option<f64>,
}

/// Statistiche delle misure con lo stesso stato VPN (`true`, `false` oppure `null` = sconosciuto).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GruppoVpn {
    pub vpn_attiva: Option<bool>,
    pub misure: u32,
    pub download: StatMisura,
    pub upload: StatMisura,
    pub ping: StatMisura,
}

fn mediana(valori: &mut [f64]) -> Option<f64> {
    if valori.is_empty() {
        return None;
    }
    valori.sort_by(|a, b| a.total_cmp(b));
    let n = valori.len();
    Some(if n % 2 == 1 {
        valori[n / 2]
    } else {
        (valori[n / 2 - 1] + valori[n / 2]) / 2.0
    })
}

fn stat(mut valori: Vec<f64>) -> StatMisura {
    let media = if valori.is_empty() {
        None
    } else {
        Some(valori.iter().sum::<f64>() / valori.len() as f64)
    };
    StatMisura {
        media,
        mediana: mediana(&mut valori),
    }
}

/// Raggruppa per stato VPN. Restituisce sempre i tre gruppi, nell'ordine acceso, spento,
/// sconosciuto; un gruppo senza misure ha `misure = 0` e statistiche vuote.
pub fn confronta_vpn(misure: &[MisuraVelocita]) -> Vec<GruppoVpn> {
    [Some(true), Some(false), None]
        .into_iter()
        .map(|stato| {
            let gruppo: Vec<&MisuraVelocita> = misure.iter().filter(|m| m.vpn_attiva == stato).collect();
            GruppoVpn {
                vpn_attiva: stato,
                misure: gruppo.len() as u32,
                download: stat(gruppo.iter().map(|m| m.download_mbps).collect()),
                upload: stat(gruppo.iter().map(|m| m.upload_mbps).collect()),
                ping: stat(gruppo.iter().map(|m| m.ping_ms).collect()),
            }
        })
        .collect()
}

impl Archivio {
    pub fn confronto_vpn(&self, da: Option<&str>, a: Option<&str>) -> Result<Vec<GruppoVpn>, String> {
        Ok(confronta_vpn(&self.storico_velocita(da, a)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(ts: &str, vpn: Option<bool>, down: f64, up: f64, ping: f64) -> MisuraVelocita {
        MisuraVelocita {
            id: None,
            timestamp: ts.into(),
            sessione: "s".into(),
            isp: None,
            ip_esterno: None,
            server_id: None,
            server_nome: None,
            server_luogo: None,
            ping_ms: ping,
            jitter_ms: None,
            download_mbps: down,
            upload_mbps: up,
            packet_loss: None,
            url: None,
            vpn_attiva: vpn,
            controllo_id: None,
            origine: "app".into(),
        }
    }

    #[test]
    fn media_e_mediana_per_gruppo() {
        let dati = vec![
            m("2026-11-01T10:00:00+01:00", Some(true), 100.0, 10.0, 20.0),
            m("2026-11-02T10:00:00+01:00", Some(true), 300.0, 30.0, 40.0),
            m("2026-11-03T10:00:00+01:00", Some(true), 200.0, 20.0, 30.0),
            m("2026-11-04T10:00:00+01:00", Some(true), 1000.0, 100.0, 90.0),
            m("2026-11-05T10:00:00+01:00", Some(false), 50.0, 5.0, 10.0),
            m("2026-11-06T10:00:00+01:00", None, 70.0, 7.0, 15.0),
        ];
        let g = confronta_vpn(&dati);
        assert_eq!(g.len(), 3);
        assert_eq!(g[0].vpn_attiva, Some(true));
        assert_eq!(g[0].misure, 4);
        assert_eq!(g[0].download.media, Some(400.0));
        assert_eq!(g[0].download.mediana, Some(250.0), "mediana di 100,200,300,1000");
        assert_eq!(g[0].upload.mediana, Some(25.0));
        assert_eq!(g[0].ping.media, Some(45.0));
        assert_eq!(g[1].vpn_attiva, Some(false));
        assert_eq!(g[1].misure, 1);
        assert_eq!(g[1].download.mediana, Some(50.0));
        assert_eq!(g[2].vpn_attiva, None);
        assert_eq!(g[2].ping.media, Some(15.0));
    }

    #[test]
    fn gruppi_vuoti_restano_presenti() {
        let g = confronta_vpn(&[m("t", Some(false), 1.0, 1.0, 1.0)]);
        assert_eq!(g[0].misure, 0);
        assert_eq!(g[0].download.media, None);
        assert_eq!(g[0].download.mediana, None);
        assert_eq!(g[1].misure, 1);
        assert!(confronta_vpn(&[]).iter().all(|x| x.misure == 0));
    }

    #[test]
    fn confronto_dal_database_con_intervallo() {
        let a = Archivio::in_memoria().unwrap();
        for (ts, vpn, down) in [
            ("2026-01-10T10:00:00+01:00", Some(true), 10.0),
            ("2026-11-10T10:00:00+01:00", Some(true), 100.0),
            ("2026-11-11T10:00:00+01:00", Some(false), 200.0),
        ] {
            a.inserisci_misura(&m(ts, vpn, down, 1.0, 1.0)).unwrap();
        }
        let tutto = a.confronto_vpn(None, None).unwrap();
        assert_eq!(tutto[0].misure, 2);
        let recente = a.confronto_vpn(Some("2026-11-01T00:00:00+01:00"), None).unwrap();
        assert_eq!(recente[0].misure, 1);
        assert_eq!(recente[0].download.media, Some(100.0));
        assert_eq!(recente[1].download.media, Some(200.0));
    }

    #[test]
    fn serializzazione_camel_case() {
        let j = serde_json::to_value(confronta_vpn(&[m("t", None, 1.0, 2.0, 3.0)])).unwrap();
        assert!(j[0]["vpnAttiva"] == true);
        assert!(j[2]["vpnAttiva"].is_null());
        assert_eq!(j[2]["download"]["media"], 1.0);
    }
}
