//! Esportazione di un controllo di sicurezza (e delle misure collegate) in un file di testo,
//! nella lingua corrente dell'app.

use std::fmt::Write as _;
use std::path::Path;

use chrono::{DateTime, Local};

use super::Archivio;
use super::sicurezza::ControlloSicurezza;
use super::velocita::MisuraVelocita;
use crate::scanner::SecurityReport;
use crate::scanner::blacklist::StatoDnsbl;
use crate::testi::{inglese, sc, tr};

fn si_no(v: bool) -> &'static str {
    if v { sc("sì", "yes") } else { "no" }
}

fn nd() -> &'static str {
    sc("N/D", "N/A")
}

/// Formato di data e ora del report: giorno/mese/anno in italiano, ISO in inglese.
fn formato_data() -> &'static str {
    if inglese() { "%Y-%m-%d %H:%M:%S" } else { "%d/%m/%Y %H:%M:%S" }
}

fn data_leggibile(iso: &str) -> String {
    DateTime::parse_from_rfc3339(iso)
        .map(|d| d.format(formato_data()).to_string())
        .unwrap_or_else(|_| iso.to_string())
}

fn verdetto_testo(codice: &str) -> &'static str {
    match codice {
        "PROTECTED" => sc("Protetta", "Protected"),
        "WARNING" => sc("Attenzione", "Warning"),
        "UNPROTECTED" => sc("Non protetta", "Unprotected"),
        "ERROR" => sc("Errore", "Error"),
        _ => sc("Sconosciuto", "Unknown"),
    }
}

fn opt(v: Option<&str>) -> &str {
    v.filter(|s| !s.is_empty()).unwrap_or(nd())
}

fn num(v: Option<f64>, cifre: usize, unita: &str) -> String {
    match v {
        Some(x) => format!("{x:.cifre$} {unita}"),
        None => nd().to_string(),
    }
}

/// Costruisce il testo del report. `generato` è l'istante di generazione (per i test).
pub fn testo_report(controllo: &ControlloSicurezza, misure: &[MisuraVelocita], generato: &str) -> String {
    let mut t = String::new();
    let _ = writeln!(t, "{}", sc("CONNESSIONE - REPORT DI SICUREZZA E VELOCITÀ", "CONNESSIONE - SECURITY AND SPEED REPORT"));
    let _ = writeln!(t, "{}", tr!("Generato il {generato}", "Generated on {generato}"));
    let _ = writeln!(t);
    let _ = writeln!(t, "{}", sc("== Controllo di sicurezza ==", "== Security check =="));
    let _ = writeln!(
        t,
        "{}",
        tr!("Data del controllo: {}", "Check date: {}", data_leggibile(&controllo.timestamp))
    );
    let _ = writeln!(
        t,
        "{}",
        tr!(
            "Verdetto: {} ({}) - punteggio {} su 100",
            "Verdict: {} ({}) - score {} out of 100",
            verdetto_testo(&controllo.verdetto),
            controllo.verdetto,
            controllo.punteggio
        )
    );

    match serde_json::from_value::<SecurityReport>(controllo.report.clone()) {
        Ok(r) => dettagli_sicurezza(&mut t, &r),
        Err(_) => {
            let _ = writeln!(t, "IPv4: {}", opt(controllo.ipv4.as_deref()));
            let _ = writeln!(
                t,
                "{}",
                tr!("VPN attiva: {}", "VPN on: {}", controllo.vpn_attiva.map(si_no).unwrap_or_else(nd))
            );
            let _ = writeln!(
                t,
                "{}",
                sc(
                    "(Il dettaglio del report non è leggibile: formato di una versione precedente.)",
                    "(The report details cannot be read: format of an earlier version.)"
                )
            );
        }
    }

    let _ = writeln!(t);
    let _ = writeln!(t, "{}", sc("== Misure di velocità collegate ==", "== Linked speed measurements =="));
    if misure.is_empty() {
        let _ = writeln!(
            t,
            "{}",
            sc(
                "Nessuna misura di velocità collegata a questo controllo.",
                "No speed measurement linked to this check."
            )
        );
    } else {
        for (i, m) in misure.iter().enumerate() {
            let _ = writeln!(
                t,
                "{}",
                tr!(
                    "{}. {} - server {} ({}) - download {:.2} Mbps, upload {:.2} Mbps, ping {:.1} ms, jitter {}, packet loss {} - VPN: {}",
                    "{}. {} - server {} ({}) - download {:.2} Mbps, upload {:.2} Mbps, ping {:.1} ms, jitter {}, packet loss {} - VPN: {}",
                    i + 1,
                    data_leggibile(&m.timestamp),
                    opt(m.server_nome.as_deref()),
                    opt(m.server_luogo.as_deref()),
                    m.download_mbps,
                    m.upload_mbps,
                    m.ping_ms,
                    num(m.jitter_ms, 1, "ms"),
                    num(m.packet_loss, 2, "%"),
                    m.vpn_attiva.map(si_no).unwrap_or_else(nd)
                )
            );
        }
        let n = misure.len() as f64;
        let _ = writeln!(
            t,
            "{}",
            tr!(
                "Media su {} misure: download {:.2} Mbps, upload {:.2} Mbps, ping {:.1} ms",
                "Average over {} measurements: download {:.2} Mbps, upload {:.2} Mbps, ping {:.1} ms",
                misure.len(),
                misure.iter().map(|m| m.download_mbps).sum::<f64>() / n,
                misure.iter().map(|m| m.upload_mbps).sum::<f64>() / n,
                misure.iter().map(|m| m.ping_ms).sum::<f64>() / n
            )
        );
    }
    t
}

fn dettagli_sicurezza(t: &mut String, r: &SecurityReport) {
    let _ = writeln!(t, "{}", tr!("Titolo: {}", "Title: {}", r.overall_title));
    let _ = writeln!(t, "{}", tr!("Riepilogo: {}", "Summary: {}", r.overall_summary));
    let _ = writeln!(t, "{}", tr!("Durata del controllo: {} s", "Check duration: {} s", r.duration_sec));

    let _ = writeln!(t, "\n{}", sc("-- Indirizzo e provider --", "-- Address and provider --"));
    let _ = writeln!(t, "IPv4: {}", opt(r.ipv4.as_deref()));
    let _ = writeln!(t, "IPv6: {}", r.ipv6.as_deref().unwrap_or(sc("nessuno", "none")));
    let _ = writeln!(t, "ISP: {}", r.isp);
    let _ = writeln!(t, "{}", tr!("Organizzazione: {}", "Organization: {}", r.org));
    let _ = writeln!(t, "ASN: {}", r.asn);
    let _ = writeln!(
        t,
        "{}",
        tr!(
            "Posizione: {}, {}, {} ({})",
            "Location: {}, {}, {} ({})",
            opt(Some(&r.city)),
            opt(Some(&r.region)),
            r.country,
            opt(Some(&r.country_code))
        )
    );
    let _ = writeln!(t, "{}", tr!("DNS inverso: {}", "Reverse DNS: {}", opt(Some(&r.reverse_dns))));
    let _ = writeln!(t, "{}", tr!("Latenza: {}", "Latency: {}", num(r.latency_ms, 1, "ms")));

    let _ = writeln!(t, "\n{}", sc("-- VPN e rete locale --", "-- VPN and local network --"));
    let _ = writeln!(
        t,
        "{}",
        tr!(
            "IP di una VPN, proxy o datacenter: {}",
            "IP of a VPN, proxy or datacenter: {}",
            if r.vpn_sconosciuta { sc("sconosciuto", "unknown") } else { si_no(r.is_vpn_or_proxy_ip) }
        )
    );
    let _ = writeln!(
        t,
        "{}",
        tr!(
            "IP di un datacenter/hosting: {}",
            "IP of a datacenter/hosting provider: {}",
            si_no(r.is_hosting_datacenter)
        )
    );
    if r.is_mullvad_server {
        let _ = writeln!(t, "{}", tr!("Server Mullvad: {}", "Mullvad server: {}", opt(Some(&r.mullvad_server_name))));
    }
    if r.active_tunnels.is_empty() {
        let _ = writeln!(t, "{}", sc("Tunnel attivi: nessuno", "Active tunnels: none"));
    } else {
        for tun in &r.active_tunnels {
            let _ = writeln!(
                t,
                "{}",
                tr!(
                    "Tunnel attivo: {} (IPv4 {}, MTU {})",
                    "Active tunnel: {} (IPv4 {}, MTU {})",
                    tun.name,
                    tun.ipv4,
                    tun.mtu
                )
            );
        }
    }
    let _ = writeln!(
        t,
        "{}",
        tr!(
            "Rotta predefinita nel tunnel: {}; protezione da caduta del tunnel: {}",
            "Default route through the tunnel: {}; tunnel drop protection: {}",
            si_no(r.default_route_via_tunnel),
            si_no(r.killswitch_or_strict_route_detected)
        )
    );
    let _ = writeln!(
        t,
        "{}",
        tr!(
            "Server DNS di sistema: {}",
            "System DNS servers: {}",
            if r.local_dns_servers.is_empty() {
                nd().to_string()
            } else {
                r.local_dns_servers.join(", ")
            }
        )
    );
    for i in &r.all_up_interfaces {
        let _ = writeln!(
            t,
            "{}",
            tr!(
                "Interfaccia: {} - IPv4 {} - IPv6 {} - MTU {}{}",
                "Interface: {} - IPv4 {} - IPv6 {} - MTU {}{}",
                i.name,
                i.ipv4,
                i.ipv6,
                i.mtu,
                if i.is_tunnel { " - tunnel" } else { "" }
            )
        );
    }

    let _ = writeln!(t, "\n{}", sc("-- Perdite (leak) --", "-- Leaks --"));
    let _ = writeln!(
        t,
        "DNS: {} ({})",
        r.dns_status_text,
        if r.dns_leak_detected { sc("leak rilevato", "leak detected") } else { sc("nessun leak", "no leak") }
    );
    for d in &r.dns_resolvers {
        let _ = writeln!(
            t,
            "{}",
            tr!(
                "Resolver DNS esterno: {} - {}{}",
                "External DNS resolver: {} - {}{}",
                d.ip,
                d.geo_org,
                d.edns_ip
                    .as_deref()
                    .map(|e| format!(" - EDNS Client Subnet: {e}"))
                    .unwrap_or_default()
            )
        );
    }
    let _ = writeln!(t, "WebRTC/STUN: {}", r.webrtc_status_text);
    let _ = writeln!(
        t,
        "IPv6: {}",
        if r.ipv6_leak_detected { sc("leak rilevato", "leak detected") } else { sc("nessun leak", "no leak") }
    );

    let _ = writeln!(t, "\n-- Blacklist (DNSBL) --");
    let verificate = r
        .blacklist_results
        .values()
        .filter(|v| v.stato != StatoDnsbl::NonVerificabile)
        .count();
    let _ = writeln!(
        t,
        "{}",
        tr!(
            "{} in lista su {} verificate",
            "{} listed out of {} checked",
            r.blacklisted_count,
            verificate
        )
    );
    for (nome, v) in &r.blacklist_results {
        let stato = match v.stato {
            StatoDnsbl::InLista => sc("IN LISTA", "LISTED"),
            StatoDnsbl::Pulito => sc("pulito", "clean"),
            StatoDnsbl::NonVerificabile => sc("non verificabile", "not verifiable"),
        };
        match &v.nota {
            Some(n) => {
                let _ = writeln!(t, "{nome}: {stato} ({n})");
            }
            None => {
                let _ = writeln!(t, "{nome}: {stato}");
            }
        }
    }

    if !r.warnings.is_empty() {
        let _ = writeln!(t, "\n{}", sc("-- Avvisi --", "-- Warnings --"));
        for w in &r.warnings {
            let _ = writeln!(t, "- {w}");
        }
    }
    if !r.positives.is_empty() {
        let _ = writeln!(t, "\n{}", sc("-- Aspetti positivi --", "-- Positives --"));
        for p in &r.positives {
            let _ = writeln!(t, "- {p}");
        }
    }
}

impl Archivio {
    /// Scrive il report del controllo `controllo_id` (l'ultimo, se `None`) in `percorso`.
    pub fn esporta_report(&self, percorso: &Path, controllo_id: Option<i64>) -> Result<(), String> {
        // Il comando puo` essere chiamato da script della pagina: solo file .txt, mai eseguibili o altro
        let estensione_ok = percorso
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("txt"));
        if !estensione_ok {
            return Err(sc(
                "Il report si può salvare solo in un file con estensione .txt",
                "The report can only be saved to a file with a .txt extension",
            )
            .into());
        }
        let controllo = self.controllo(controllo_id)?.ok_or_else(|| {
            sc("Nessun controllo di sicurezza da esportare", "No security check to export").to_string()
        })?;
        let misure = self.misure_per_controllo(controllo.id)?;
        let generato = Local::now().format(formato_data()).to_string();
        std::fs::write(percorso, testo_report(&controllo, &misure, &generato))
            .map_err(|e| tr!("Impossibile scrivere {}: {e}", "Unable to write {}: {e}", percorso.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::blacklist::VoceDnsbl;

    fn report_di_prova() -> SecurityReport {
        let mut r = SecurityReport::vuoto("2026-11-23T10:20:59+01:00".into());
        r.overall_status = crate::scanner::report::OverallStatus::Unprotected;
        r.overall_title = "CONNESSIONE NON PROTETTA (NESSUNA VPN ATTIVA)".into();
        r.security_score = 35;
        r.ipv4 = Some("203.0.113.7".into());
        r.isp = "Fastweb".into();
        r.country = "Italy".into();
        r.city = "Roma".into();
        r.local_dns_servers = vec!["192.168.1.1".into()];
        r.blacklisted_count = 0;
        r.blacklist_results.insert(
            "Spamhaus ZEN".into(),
            VoceDnsbl::nuova(StatoDnsbl::Pulito, Some("IP residenziale/dinamico (PBL)".into())),
        );
        r.blacklist_results
            .insert("SpamCop".into(), VoceDnsbl::nuova(StatoDnsbl::NonVerificabile, None));
        r.warnings.push("NON SEI SU VPN".into());
        r
    }

    fn controllo(r: &SecurityReport) -> ControlloSicurezza {
        ControlloSicurezza {
            id: 7,
            timestamp: r.timestamp.clone(),
            verdetto: "UNPROTECTED".into(),
            punteggio: 35,
            ipv4: r.ipv4.clone(),
            vpn_attiva: Some(false),
            report: serde_json::to_value(r).unwrap(),
        }
    }

    fn misura() -> MisuraVelocita {
        MisuraVelocita {
            id: Some(1),
            timestamp: "2026-11-23T10:21:30+01:00".into(),
            sessione: "s".into(),
            isp: None,
            ip_esterno: None,
            server_id: Some(1),
            server_nome: Some("Server Esempio".into()),
            server_luogo: Some("Milano".into()),
            ping_ms: 21.5,
            jitter_ms: Some(1.25),
            download_mbps: 100.0,
            upload_mbps: 12.0,
            packet_loss: None,
            url: None,
            vpn_attiva: Some(false),
            controllo_id: Some(7),
            origine: "app".into(),
        }
    }

    #[test]
    fn testo_con_sicurezza_e_misure() {
        let r = report_di_prova();
        let t = testo_report(&controllo(&r), &[misura()], "23/11/2026 10:30:00");
        assert!(t.starts_with("CONNESSIONE - REPORT DI SICUREZZA E VELOCITÀ\nGenerato il 23/11/2026 10:30:00\n"));
        assert!(t.contains("Data del controllo: 23/11/2026 10:20:59"));
        assert!(t.contains("Verdetto: Non protetta (UNPROTECTED) - punteggio 35 su 100"));
        assert!(t.contains("IPv4: 203.0.113.7"));
        assert!(t.contains("Server DNS di sistema: 192.168.1.1"));
        assert!(t.contains("0 in lista su 1 verificate"));
        assert!(t.contains("Spamhaus ZEN: pulito (IP residenziale/dinamico (PBL))"));
        assert!(t.contains("SpamCop: non verificabile"));
        assert!(t.contains("- NON SEI SU VPN"));
        assert!(t.contains("1. 23/11/2026 10:21:30 - server Server Esempio (Milano) - download 100.00 Mbps, upload 12.00 Mbps, ping 21.5 ms, jitter 1.2 ms, packet loss N/D - VPN: no"));
        assert!(t.contains("Media su 1 misure: download 100.00 Mbps"));
    }

    #[test]
    fn esportazione_solo_in_file_txt() {
        let a = Archivio::in_memoria().unwrap();
        a.salva_controllo(&report_di_prova()).unwrap();
        let dir = tempfile::tempdir().unwrap();
        for nome in ["report.exe", "report.csv", "report", "report.txt.bat", "report.tx"] {
            let p = dir.path().join(nome);
            let e = a.esporta_report(&p, None);
            assert!(e.is_err(), "{nome} doveva essere rifiutato");
            assert!(!p.exists(), "{nome} non deve essere stato scritto");
        }
        for nome in ["report.txt", "REPORT.TXT", "Report.Txt"] {
            let p = dir.path().join(nome);
            a.esporta_report(&p, None).unwrap();
            assert!(p.exists());
        }
    }

    #[test]
    fn testo_senza_misure_e_con_report_illeggibile() {
        let r = report_di_prova();
        let mut c = controllo(&r);
        c.report = serde_json::json!({"blacklistResults": {"X": true}});
        let t = testo_report(&c, &[], "ora");
        assert!(t.contains("Nessuna misura di velocità collegata a questo controllo."));
        assert!(t.contains("formato di una versione precedente"));
        assert!(t.contains("VPN attiva: no"));
    }

    #[test]
    fn esporta_su_file_l_ultimo_controllo_con_le_misure_collegate() {
        let a = Archivio::in_memoria().unwrap();
        assert!(a.esporta_report(Path::new("x.txt"), None).is_err(), "nessun controllo");
        let r = report_di_prova();
        let id = a.salva_controllo(&r).unwrap();
        let mut m = misura();
        m.controllo_id = Some(id);
        a.inserisci_misura(&m).unwrap();
        let mut altra = misura();
        altra.controllo_id = None;
        a.inserisci_misura(&altra).unwrap();
        assert_eq!(a.misure_per_controllo(id).unwrap().len(), 1);

        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("report.txt");
        a.esporta_report(&file, None).unwrap();
        let t = std::fs::read_to_string(&file).unwrap();
        assert!(t.contains("== Controllo di sicurezza =="));
        assert!(t.contains("Media su 1 misure"));
        assert!(a.esporta_report(&file, Some(999)).is_err());
        assert_eq!(a.ultimi_controlli(10).unwrap().len(), 1);
    }
}

#[cfg(test)]
mod tests_lingua {
    use super::*;
    use crate::testi::{LinguaEffettiva, con_lingua};

    #[test]
    fn report_esportato_nelle_due_lingue() {
        let c = ControlloSicurezza {
            id: 1,
            timestamp: "2026-11-23T10:00:00+01:00".into(),
            verdetto: "PROTECTED".into(),
            punteggio: 90,
            ipv4: Some("203.0.113.7".into()),
            vpn_attiva: Some(true),
            report: serde_json::Value::Null,
        };
        let it = con_lingua(LinguaEffettiva::It, || testo_report(&c, &[], "x"));
        let en = con_lingua(LinguaEffettiva::En, || testo_report(&c, &[], "x"));
        assert!(it.contains("Verdetto: Protetta") && it.contains("23/11/2026 10:00:00"));
        assert!(en.contains("Verdict: Protected") && en.contains("2026-11-23 10:00:00"));
        assert!(en.contains("No speed measurement linked"));
    }
}
