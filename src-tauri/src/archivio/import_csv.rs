//! Import del CSV storico dello script PowerShell (`speedtest_log.csv`), scelto dall'utente.
//!
//! Intestazione: Timestamp,ISP,Server,Ping_ms,Download_Mbps,Upload_Mbps,PacketLoss,URL.
//! Decimali con la virgola, PacketLoss a volte vuoto. Il file non viene mai modificato.

use std::path::Path;

use chrono::{Local, NaiveDateTime, SecondsFormat, TimeZone};
use serde::Serialize;

use super::velocita::{MisuraVelocita, esiste_su, inserisci_su};
use super::Archivio;
use crate::testi::{sc, tr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RisultatoImport {
    pub importate: u32,
    pub duplicate: u32,
    pub scartate: u32,
}

impl RisultatoImport {
    /// «N misure importate, M già presenti» (più le righe scartate, se ce ne sono).
    pub fn messaggio(&self) -> String {
        let mut m = tr!("{} misure importate, {} già presenti", "{} measurements imported, {} already present", self.importate, self.duplicate);
        if self.scartate > 0 {
            m.push_str(&tr!(" ({} non valide scartate)", " ({} invalid discarded)", self.scartate));
        }
        m
    }
}

/// "21,16" -> 21.16; vuoto o non numerico -> None.
fn decimale(s: &str) -> Option<f64> {
    let t = s.trim().replace(',', ".");
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// "2026-04-10 11:19:03" (ora locale) -> ISO 8601 con offset locale.
fn timestamp_iso(s: &str) -> Option<String> {
    let naive = NaiveDateTime::parse_from_str(s.trim(), "%Y-%m-%d %H:%M:%S").ok()?;
    let locale = Local.from_local_datetime(&naive).earliest()?;
    Some(locale.to_rfc3339_opts(SecondsFormat::Secs, false))
}

fn non_vuoto(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

impl Archivio {
    /// Importa il CSV in modo idempotente: una riga con lo stesso timestamp e url è un duplicato.
    pub fn importa_csv(&self, percorso: &Path) -> Result<RisultatoImport, String> {
        let mut lettore = csv::ReaderBuilder::new()
            .flexible(true)
            .from_path(percorso)
            .map_err(|e| tr!("Impossibile aprire {}: {e}", "Unable to open {}: {e}", percorso.display()))?;
        let intestazione = lettore.headers().map_err(|e| e.to_string())?.clone();
        let colonna = |nome: &str| {
            intestazione
                .iter()
                .position(|h| h.trim_start_matches('\u{feff}').trim().eq_ignore_ascii_case(nome))
        };
        let (Some(c_ts), Some(c_ping), Some(c_down), Some(c_up)) = (
            colonna("Timestamp"),
            colonna("Ping_ms"),
            colonna("Download_Mbps"),
            colonna("Upload_Mbps"),
        ) else {
            return Err(sc("Intestazione CSV non riconosciuta (attese Timestamp, Ping_ms, Download_Mbps, Upload_Mbps)", "CSV header not recognised (expected Timestamp, Ping_ms, Download_Mbps, Upload_Mbps)").into());
        };
        let (c_isp, c_server, c_loss, c_url) = (
            colonna("ISP"),
            colonna("Server"),
            colonna("PacketLoss"),
            colonna("URL"),
        );

        // Tutto in una sola transazione: o si importa l'intero file o niente (un errore a metà
        // non lascia un database con mezzo storico, che l'import successivo non completerebbe).
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        let mut esito = RisultatoImport::default();
        for record in lettore.records() {
            let Ok(record) = record else {
                esito.scartate += 1;
                continue;
            };
            let campo = |i: Option<usize>| i.and_then(|i| record.get(i)).unwrap_or("");
            let (Some(timestamp), Some(ping), Some(down), Some(up)) = (
                timestamp_iso(campo(Some(c_ts))),
                decimale(campo(Some(c_ping))),
                decimale(campo(Some(c_down))),
                decimale(campo(Some(c_up))),
            ) else {
                esito.scartate += 1;
                continue;
            };
            let url = non_vuoto(campo(c_url));
            if esiste_su(&tx, &timestamp, url.as_deref())? {
                esito.duplicate += 1;
                continue;
            }
            inserisci_su(&tx, &MisuraVelocita {
                id: None,
                timestamp,
                sessione: uuid::Uuid::new_v4().to_string(),
                isp: non_vuoto(campo(c_isp)),
                ip_esterno: None,
                server_id: None,
                server_nome: non_vuoto(campo(c_server)),
                server_luogo: None,
                ping_ms: ping,
                jitter_ms: None,
                download_mbps: down,
                upload_mbps: up,
                packet_loss: decimale(campo(c_loss)),
                url,
                vpn_attiva: None,
                controllo_id: None,
                origine: "import".into(),
            })?;
            esito.importate += 1;
        }
        tx.commit().map_err(|e| e.to_string())?;
        Ok(esito)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const CSV_VERO: &str = include_str!("../../tests/fixtures/speedtest_log.csv");

    /// Copia del CSV vero in una cartella temporanea (l'originale non si tocca mai).
    fn copia_csv(dir: &tempfile::TempDir, contenuto: &str) -> std::path::PathBuf {
        let p = dir.path().join("speedtest_log.csv");
        fs::write(&p, contenuto).unwrap();
        p
    }

    #[test]
    fn messaggio_dell_esito() {
        let r = RisultatoImport { importate: 12, duplicate: 3, scartate: 0 };
        assert_eq!(r.messaggio(), "12 misure importate, 3 già presenti");
        let r = RisultatoImport { importate: 1, duplicate: 0, scartate: 2 };
        assert_eq!(r.messaggio(), "1 misure importate, 0 già presenti (2 non valide scartate)");
    }

    #[test]
    fn decimali_con_la_virgola() {
        assert_eq!(decimale("21,16"), Some(21.16));
        assert_eq!(decimale("0"), Some(0.0));
        assert_eq!(decimale("1,8691588785046729"), Some(1.8691588785046729));
        assert_eq!(decimale(""), None);
        assert_eq!(decimale("abc"), None);
    }

    #[test]
    fn timestamp_in_iso_con_offset() {
        let t = timestamp_iso("2026-04-10 11:19:03").unwrap();
        assert!(t.starts_with("2026-04-10T11:19:03"));
        assert!(t.len() > 19, "deve avere l'offset: {t}");
        assert!(timestamp_iso("ieri").is_none());
    }

    #[test]
    fn import_del_csv_vero_e_idempotente() {
        let dir = tempfile::tempdir().unwrap();
        let path = copia_csv(&dir, CSV_VERO);
        let righe_dati = CSV_VERO.lines().skip(1).filter(|l| !l.trim().is_empty()).count() as u32;
        assert_eq!(righe_dati, 13);

        let a = Archivio::in_memoria().unwrap();
        let r1 = a.importa_csv(&path).unwrap();
        assert_eq!(r1, RisultatoImport { importate: 13, duplicate: 0, scartate: 0 });
        assert_eq!(a.conta_misure().unwrap(), 13);

        let r2 = a.importa_csv(&path).unwrap();
        assert_eq!(r2, RisultatoImport { importate: 0, duplicate: 13, scartate: 0 });
        assert_eq!(a.conta_misure().unwrap(), 13);

        // Il file copiato non è stato alterato dall'import
        assert_eq!(fs::read_to_string(&path).unwrap(), CSV_VERO);
    }

    #[test]
    fn valori_della_prima_e_dell_ultima_riga() {
        let dir = tempfile::tempdir().unwrap();
        let path = copia_csv(&dir, CSV_VERO);
        let a = Archivio::in_memoria().unwrap();
        a.importa_csv(&path).unwrap();
        let v = a.storico_velocita(None, None).unwrap();
        let primo = &v[0];
        assert!(primo.timestamp.starts_with("2026-04-10T11:19:03"));
        assert_eq!(primo.isp.as_deref(), Some("Provider Esempio"));
        assert_eq!(primo.server_nome.as_deref(), Some("Server Esempio – Milano"));
        assert_eq!(primo.ping_ms, 21.16);
        assert_eq!(primo.download_mbps, 82.09);
        assert_eq!(primo.upload_mbps, 17.11);
        assert_eq!(primo.packet_loss, Some(1.8691588785046729));
        assert_eq!(primo.origine, "import");
        assert_eq!(primo.vpn_attiva, None);
        assert_eq!(primo.jitter_ms, None);
        assert!(primo.url.as_deref().unwrap().starts_with("https://www.speedtest.net/result/c/00000000-0000-0000-0000-000000000001"));
        let ultimo = v.last().unwrap();
        assert!(ultimo.timestamp.starts_with("2026-11-09T17:26:52"));
        // Server con virgola dentro le virgolette
        assert!(v.iter().any(|m| m.server_nome.as_deref() == Some("Server Esempio, S.r.l.")));
    }

    #[test]
    fn packet_loss_vuoto_diventa_null_e_righe_rotte_scartate() {
        let csv = "Timestamp,ISP,Server,Ping_ms,Download_Mbps,Upload_Mbps,PacketLoss,URL\r\n\
                   \"2026-05-01 10:00:00\",\"X\",\"S\",\"10,5\",\"100,25\",\"20\",\"\",\"https://u/1\"\r\n\
                   \"non-una-data\",\"X\",\"S\",\"10\",\"1\",\"1\",\"0\",\"https://u/2\"\r\n\
                   \"2026-05-02 10:00:00\",\"X\",\"S\",\"\",\"1\",\"1\",\"0\",\"https://u/3\"\r\n";
        let dir = tempfile::tempdir().unwrap();
        let path = copia_csv(&dir, csv);
        let a = Archivio::in_memoria().unwrap();
        let r = a.importa_csv(&path).unwrap();
        assert_eq!(r, RisultatoImport { importate: 1, duplicate: 0, scartate: 2 });
        let v = a.storico_velocita(None, None).unwrap();
        assert_eq!(v[0].packet_loss, None);
        assert_eq!(v[0].download_mbps, 100.25);
    }

    #[test]
    fn intestazione_sconosciuta_e_un_errore() {
        let dir = tempfile::tempdir().unwrap();
        let path = copia_csv(&dir, "a,b,c\n1,2,3\n");
        let a = Archivio::in_memoria().unwrap();
        assert!(a.importa_csv(&path).is_err());
    }

    #[test]
    fn file_inesistente_e_un_errore() {
        let a = Archivio::in_memoria().unwrap();
        assert!(a.importa_csv(Path::new("C:\\non\\esiste.csv")).is_err());
    }

    #[test]
    fn import_atomico_un_errore_a_meta_non_lascia_nulla() {
        let a = Archivio::in_memoria().unwrap();
        // Trigger di prova: l'inserimento della seconda riga (download 999) fallisce
        a.conn()
            .execute_batch(
                "CREATE TRIGGER blocca BEFORE INSERT ON misure_velocita WHEN NEW.download_mbps = 999
                 BEGIN SELECT RAISE(ABORT, 'riga rifiutata'); END;",
            )
            .unwrap();
        let csv = "Timestamp,ISP,Server,Ping_ms,Download_Mbps,Upload_Mbps,PacketLoss,URL\r\n\
                   \"2026-05-01 10:00:00\",\"X\",\"S\",\"10\",\"100\",\"20\",\"0\",\"https://u/1\"\r\n\
                   \"2026-05-02 10:00:00\",\"X\",\"S\",\"10\",\"999\",\"20\",\"0\",\"https://u/2\"\r\n";
        let dir = tempfile::tempdir().unwrap();
        let path = copia_csv(&dir, csv);
        assert!(a.importa_csv(&path).is_err());
        assert_eq!(a.conta_misure().unwrap(), 0, "la prima riga non deve restare importata");
        // Tolto l'ostacolo, un secondo tentativo importa tutto
        a.conn().execute_batch("DROP TRIGGER blocca;").unwrap();
        assert_eq!(a.importa_csv(&path).unwrap().importate, 2);
    }
}
