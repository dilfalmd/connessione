//! Logica pura delle notifiche: quali avvisi generare passando dal controllo precedente al nuovo.
//!
//! Porta in Rust `_check_transition_alerts` di sicurezza_connessione/app.py (VPN caduta, cambio di
//! IP) e aggiunge peggioramento del verdetto, perdite DNS/WebRTC, nuove blacklist, soglie di
//! velocità e ripristino della VPN. La stessa notifica non si ripete entro 30 minuti.

use std::collections::HashMap;

use crate::archivio::MisuraVelocita;
use crate::scanner::blacklist::StatoDnsbl;
use crate::scanner::report::{OverallStatus, SecurityReport};
use crate::testi::{sc, tr};

pub const FINESTRA_ANTIRIPETIZIONE_SEC: i64 = 30 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TipoAvviso {
    VpnCaduta,
    VerdettoPeggiora,
    LeakDns,
    LeakWebrtc,
    IpCambiato,
    BlacklistNuova,
    DownloadBasso,
    PingAlto,
    VpnRipristinata,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Avviso {
    pub tipo: TipoAvviso,
    pub titolo: String,
    pub testo: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Soglie {
    pub download_min_mbps: Option<f64>,
    pub ping_max_ms: Option<f64>,
}

/// Memoria fra un controllo e l'altro.
#[derive(Debug, Default)]
pub struct StatoNotifiche {
    ultimo_report: Option<SecurityReport>,
    ultima_misura: Option<MisuraVelocita>,
    /// La VPN è caduta e non è ancora tornata: serve per la notifica di ripristino.
    caduta_pendente: bool,
    /// Istante (secondi) dell'ultima notifica inviata per tipo.
    inviati: HashMap<TipoAvviso, i64>,
    /// Notifiche disattivate: gli avvisi si scartano senza registrarli come inviati, così
    /// riattivandole non si resta zitti per 30 minuti per avvisi che nessuno ha visto.
    pub silenziate: bool,
}

fn peso(s: OverallStatus) -> Option<u8> {
    match s {
        OverallStatus::Protected => Some(0),
        OverallStatus::Warning => Some(1),
        OverallStatus::Unprotected => Some(2),
        OverallStatus::Error => None,
    }
}

pub fn etichetta_verdetto(s: OverallStatus) -> &'static str {
    match s {
        OverallStatus::Protected => sc("Protetta", "Protected"),
        OverallStatus::Warning => sc("Attenzione", "Warning"),
        OverallStatus::Unprotected => sc("Non protetta", "Unprotected"),
        OverallStatus::Error => sc("Errore", "Error"),
    }
}

fn ip_e_isp(r: &SecurityReport) -> String {
    format!("{} ({})", r.ipv4.as_deref().unwrap_or(sc("N/D", "N/A")), r.isp)
}

fn avviso(tipo: TipoAvviso, titolo: &str, testo: String) -> Avviso {
    Avviso { tipo, titolo: titolo.to_string(), testo }
}

impl StatoNotifiche {
    /// Imposta il punto di partenza senza generare avvisi (usato dai test).
    #[cfg(test)]
    pub fn inizializza(&mut self, report: Option<SecurityReport>, misura: Option<MisuraVelocita>) {
        self.ultimo_report = report;
        self.ultima_misura = misura;
    }

    /// Scarta i tipi già notificati negli ultimi 30 minuti e registra gli altri.
    fn filtra(&mut self, avvisi: Vec<Avviso>, ora: i64) -> Vec<Avviso> {
        if self.silenziate {
            return Vec::new();
        }
        let mut risultato = Vec::new();
        for a in avvisi {
            let recente = self
                .inviati
                .get(&a.tipo)
                .is_some_and(|&ultimo| ora - ultimo < FINESTRA_ANTIRIPETIZIONE_SEC);
            if !recente {
                self.inviati.insert(a.tipo, ora);
                risultato.push(a);
            }
        }
        risultato
    }

    /// Confronta il nuovo controllo di sicurezza con il precedente. `ora` in secondi.
    pub fn controllo(&mut self, curr: &SecurityReport, ora: i64) -> Vec<Avviso> {
        // Controllo fallito (nessun IP): non è un cambiamento di stato, si conserva il precedente
        if curr.ipv4.is_none() {
            return Vec::new();
        }
        let Some(prev) = self.ultimo_report.replace(curr.clone()) else {
            return Vec::new();
        };

        let mut v = Vec::new();

        // Se uno dei due controlli non sa se c'e` una VPN (dato ignoto e nessun tunnel locale che
        // decida), non si puo` dire che sia caduta o tornata: nessun avviso su questo fronte.
        let incerto = prev.vpn_sconosciuta || curr.vpn_sconosciuta;

        // VPN da accesa a spenta (o, come nel Python, da protetta/attenzione a non protetta)
        let caduta = !incerto
            && ((prev.is_vpn_or_proxy_ip && !curr.is_vpn_or_proxy_ip)
                || (matches!(prev.overall_status, OverallStatus::Protected | OverallStatus::Warning)
                    && curr.overall_status == OverallStatus::Unprotected));
        if caduta {
            self.caduta_pendente = true;
            v.push(avviso(
                TipoAvviso::VpnCaduta,
                sc("La protezione VPN è caduta", "VPN protection dropped"),
                tr!(
                    "IP precedente: {}\nIP attuale: {}",
                    "Previous IP: {}\nCurrent IP: {}",
                    ip_e_isp(&prev),
                    ip_e_isp(curr)
                ),
            ));
        } else if let (Some(p), Some(c)) = (peso(prev.overall_status), peso(curr.overall_status))
            && c > p
        {
            v.push(avviso(
                TipoAvviso::VerdettoPeggiora,
                sc("Il verdetto è peggiorato", "The verdict got worse"),
                tr!(
                    "Da «{}» a «{}»: {}",
                    "From “{}” to “{}”: {}",
                    etichetta_verdetto(prev.overall_status),
                    etichetta_verdetto(curr.overall_status),
                    curr.overall_title
                ),
            ));
        }

        // Compare una perdita DNS o WebRTC
        if !prev.dns_leak_detected && curr.dns_leak_detected {
            v.push(avviso(
                TipoAvviso::LeakDns,
                sc("Perdita DNS rilevata", "DNS leak detected"),
                curr.dns_status_text.clone(),
            ));
        }
        if !prev.webrtc_leak_detected && curr.webrtc_leak_detected {
            v.push(avviso(
                TipoAvviso::LeakWebrtc,
                sc("Perdita WebRTC rilevata", "WebRTC leak detected"),
                curr.webrtc_status_text.clone(),
            ));
        }

        // L'IP pubblico cambia mentre la VPN è accesa (prima e dopo)
        if prev.is_vpn_or_proxy_ip
            && curr.is_vpn_or_proxy_ip
            && let (Some(a), Some(b)) = (&prev.ipv4, &curr.ipv4)
            && a != b
        {
            v.push(avviso(
                TipoAvviso::IpCambiato,
                sc("Cambio di indirizzo IP con la VPN accesa", "IP address changed while the VPN is on"),
                tr!("Prima: {}\nAdesso: {}", "Before: {}\nNow: {}", ip_e_isp(&prev), ip_e_isp(curr)),
            ));
        }

        // Un IP entra in una blacklist (solo stato in_lista)
        let nuove: Vec<&str> = curr
            .blacklist_results
            .iter()
            .filter(|(nome, voce)| {
                voce.stato == StatoDnsbl::InLista
                    && prev
                        .blacklist_results
                        .get(*nome)
                        .is_none_or(|p| p.stato != StatoDnsbl::InLista)
            })
            .map(|(nome, _)| nome.as_str())
            .collect();
        if !nuove.is_empty() {
            v.push(avviso(
                TipoAvviso::BlacklistNuova,
                sc("IP in blacklist", "IP blacklisted"),
                tr!(
                    "L'indirizzo {} è comparso in: {}",
                    "Address {} appeared in: {}",
                    curr.ipv4.as_deref().unwrap_or(sc("N/D", "N/A")),
                    nuove.join(", ")
                ),
            ));
        }

        // La VPN torna accesa dopo una caduta
        if !caduta && !incerto && self.caduta_pendente && curr.is_vpn_or_proxy_ip {
            self.caduta_pendente = false;
            v.push(avviso(
                TipoAvviso::VpnRipristinata,
                sc("VPN ripristinata", "VPN restored"),
                tr!("La VPN è di nuovo attiva: {}", "The VPN is active again: {}", ip_e_isp(curr)),
            ));
        }

        self.filtra(v, ora)
    }

    /// Confronta una nuova misura di velocità con la precedente: avvisa quando si passa
    /// da dentro a fuori soglia (non a ogni misura già fuori soglia).
    pub fn misura(&mut self, m: &MisuraVelocita, soglie: Soglie, ora: i64) -> Vec<Avviso> {
        let prev = self.ultima_misura.replace(m.clone());
        let mut v = Vec::new();

        if let Some(min) = soglie.download_min_mbps
            && m.download_mbps < min
            && !prev.as_ref().is_some_and(|p| p.download_mbps < min)
        {
            v.push(avviso(
                TipoAvviso::DownloadBasso,
                sc("Download sotto soglia", "Download below threshold"),
                tr!("{:.1} Mbps, soglia minima {:.1} Mbps", "{:.1} Mbps, minimum threshold {:.1} Mbps", m.download_mbps, min),
            ));
        }
        if let Some(max) = soglie.ping_max_ms
            && m.ping_ms > max
            && !prev.as_ref().is_some_and(|p| p.ping_ms > max)
        {
            v.push(avviso(
                TipoAvviso::PingAlto,
                sc("Ping sopra soglia", "Ping above threshold"),
                tr!("{:.1} ms, soglia massima {:.1} ms", "{:.1} ms, maximum threshold {:.1} ms", m.ping_ms, max),
            ));
        }
        self.filtra(v, ora)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::blacklist::VoceDnsbl;

    fn rep(vpn: bool, stato: OverallStatus, ip: &str) -> SecurityReport {
        let mut r = SecurityReport::vuoto("2026-11-23T10:00:00+01:00".into());
        r.ipv4 = Some(ip.into());
        r.isp = "Isp".into();
        r.is_vpn_or_proxy_ip = vpn;
        r.overall_status = stato;
        r.overall_title = "titolo".into();
        r
    }

    fn con_stato(prev: SecurityReport) -> StatoNotifiche {
        let mut s = StatoNotifiche::default();
        s.inizializza(Some(prev), None);
        s
    }

    fn tipi(v: &[Avviso]) -> Vec<TipoAvviso> {
        v.iter().map(|a| a.tipo).collect()
    }

    fn misura(down: f64, ping: f64) -> MisuraVelocita {
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
            jitter_ms: None,
            download_mbps: down,
            upload_mbps: 1.0,
            packet_loss: None,
            url: None,
            vpn_attiva: None,
            controllo_id: None,
            origine: "app".into(),
        }
    }

    #[test]
    fn primo_controllo_senza_precedente_non_avvisa() {
        let mut s = StatoNotifiche::default();
        assert!(s.controllo(&rep(false, OverallStatus::Unprotected, "1.1.1.1"), 0).is_empty());
        // ...ma diventa il riferimento per il successivo
        let v = s.controllo(&rep(false, OverallStatus::Unprotected, "2.2.2.2"), 10);
        assert!(v.is_empty(), "IP cambiato senza VPN non e` un avviso");
    }

    fn ignota(mut r: SecurityReport) -> SecurityReport {
        r.vpn_sconosciuta = true;
        r.is_vpn_or_proxy_ip = false;
        r
    }

    #[test]
    fn vpn_ignota_non_genera_ne_caduta_ne_ripristino() {
        // accesa -> ignota: nessuna caduta (non e` detto che sia caduta)
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let v = s.controllo(&ignota(rep(false, OverallStatus::Unprotected, "9.9.9.9")), 0);
        assert!(!tipi(&v).contains(&TipoAvviso::VpnCaduta), "{:?}", tipi(&v));
        // ignota -> accesa: nessun ripristino, e la caduta pendente non viene consumata
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 0); // caduta vera
        s.controllo(&ignota(rep(false, OverallStatus::Unprotected, "5.5.5.5")), 100);
        let v = s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 200);
        assert!(v.is_empty(), "prev ignoto: nessun ripristino ({:?})", tipi(&v));
        // ...ma con due dati noti il ripristino arriva
        let v = s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 3000);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnCaduta]);
        let v = s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 3100);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnRipristinata]);
    }

    #[test]
    fn vpn_ignota_con_tunnel_locale_che_decide_non_e_ignota() {
        // Con un tunnel locale vpn_sconosciuta e` false: la caduta si vede normalmente
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut c = rep(false, OverallStatus::Unprotected, "5.5.5.5");
        c.vpn_sconosciuta = false;
        assert_eq!(tipi(&s.controllo(&c, 0)), vec![TipoAvviso::VpnCaduta]);
    }

    #[test]
    fn notifiche_silenziate_non_si_registrano_come_inviate() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut leak = rep(true, OverallStatus::Protected, "9.9.9.9");
        leak.dns_leak_detected = true;
        let pulito = rep(true, OverallStatus::Protected, "9.9.9.9");

        s.silenziate = true;
        assert!(s.controllo(&leak, 0).is_empty(), "silenziate: nessun avviso");
        s.controllo(&pulito, 60);
        // Riattivate: lo stesso avviso, pochi secondi dopo, arriva (non e` stato "inviato")
        s.silenziate = false;
        assert_eq!(tipi(&s.controllo(&leak, 120)), vec![TipoAvviso::LeakDns]);
    }

    #[test]
    fn misure_silenziate_non_si_registrano_come_inviate() {
        let soglie = Soglie { download_min_mbps: Some(50.0), ping_max_ms: None };
        let mut s = StatoNotifiche { silenziate: true, ..Default::default() };
        assert!(s.misura(&misura(10.0, 1.0), soglie, 0).is_empty());
        s.misura(&misura(90.0, 1.0), soglie, 10);
        s.silenziate = false;
        assert_eq!(tipi(&s.misura(&misura(10.0, 1.0), soglie, 20)), vec![TipoAvviso::DownloadBasso]);
    }

    #[test]
    fn nessun_cambiamento_nessun_avviso() {
        let r = rep(true, OverallStatus::Protected, "9.9.9.9");
        let mut s = con_stato(r.clone());
        assert!(s.controllo(&r, 0).is_empty());
    }

    #[test]
    fn vpn_da_accesa_a_spenta() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let v = s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 0);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnCaduta], "il peggioramento del verdetto non si raddoppia");
        assert!(v[0].testo.contains("9.9.9.9") && v[0].testo.contains("5.5.5.5"));
    }

    #[test]
    fn caso_python_protetta_a_non_protetta_anche_se_il_flag_vpn_resta() {
        let mut s = con_stato(rep(true, OverallStatus::Warning, "9.9.9.9"));
        let v = s.controllo(&rep(true, OverallStatus::Unprotected, "9.9.9.9"), 0);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnCaduta]);
    }

    #[test]
    fn verdetto_che_peggiora() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let v = s.controllo(&rep(true, OverallStatus::Warning, "9.9.9.9"), 0);
        assert_eq!(tipi(&v), vec![TipoAvviso::VerdettoPeggiora]);
        assert!(v[0].testo.contains("Protetta") && v[0].testo.contains("Attenzione"));
        // un miglioramento non avvisa
        let v = s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 5000);
        assert!(v.is_empty());
    }

    #[test]
    fn controllo_in_errore_non_altera_lo_stato() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut errore = SecurityReport::vuoto("t".into());
        errore.overall_status = OverallStatus::Error;
        assert!(s.controllo(&errore, 0).is_empty());
        // Il riferimento e` ancora il report protetto
        let v = s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 10);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnCaduta]);
    }

    #[test]
    fn compare_una_perdita_dns() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut c = rep(true, OverallStatus::Protected, "9.9.9.9");
        c.dns_leak_detected = true;
        c.dns_status_text = "POSSIBILE DNS LEAK".into();
        let v = s.controllo(&c, 0);
        assert_eq!(tipi(&v), vec![TipoAvviso::LeakDns]);
        // una perdita gia` presente non rinotifica (nemmeno dopo 30 min: non e` una transizione)
        assert!(s.controllo(&c, 99_999).is_empty());
    }

    #[test]
    fn compare_una_perdita_webrtc() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut c = rep(true, OverallStatus::Protected, "9.9.9.9");
        c.webrtc_leak_detected = true;
        c.webrtc_status_text = "LEAK RILEVATO".into();
        assert_eq!(tipi(&s.controllo(&c, 0)), vec![TipoAvviso::LeakWebrtc]);
    }

    #[test]
    fn ip_cambia_con_la_vpn_accesa() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let v = s.controllo(&rep(true, OverallStatus::Protected, "8.8.8.8"), 0);
        assert_eq!(tipi(&v), vec![TipoAvviso::IpCambiato]);
        assert!(v[0].testo.contains("9.9.9.9") && v[0].testo.contains("8.8.8.8"));
    }

    #[test]
    fn ip_cambia_con_la_vpn_spenta_non_avvisa() {
        let mut s = con_stato(rep(false, OverallStatus::Unprotected, "9.9.9.9"));
        assert!(s.controllo(&rep(false, OverallStatus::Unprotected, "8.8.8.8"), 0).is_empty());
    }

    #[test]
    fn un_ip_entra_in_blacklist() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut c = rep(true, OverallStatus::Protected, "9.9.9.9");
        c.blacklist_results.insert("Spamhaus ZEN".into(), VoceDnsbl::nuova(StatoDnsbl::InLista, None));
        c.blacklist_results.insert("SpamCop".into(), VoceDnsbl::nuova(StatoDnsbl::NonVerificabile, None));
        c.blacklist_results.insert("Barracuda".into(), VoceDnsbl::nuova(StatoDnsbl::Pulito, Some("PBL".into())));
        let v = s.controllo(&c, 0);
        assert_eq!(tipi(&v), vec![TipoAvviso::BlacklistNuova]);
        assert!(v[0].testo.contains("Spamhaus ZEN") && !v[0].testo.contains("SpamCop") && !v[0].testo.contains("Barracuda"));
        // gia` in lista: nessuna nuova notifica
        assert!(s.controllo(&c, 99_999).is_empty());
    }

    #[test]
    fn pbl_e_non_verificabile_non_sono_blacklist() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut c = rep(true, OverallStatus::Protected, "9.9.9.9");
        c.blacklist_results.insert("Spamhaus ZEN".into(), VoceDnsbl::nuova(StatoDnsbl::Pulito, Some("PBL".into())));
        c.blacklist_results.insert("SpamCop".into(), VoceDnsbl::nuova(StatoDnsbl::NonVerificabile, None));
        assert!(s.controllo(&c, 0).is_empty());
    }

    #[test]
    fn ripristino_solo_dopo_una_caduta() {
        let mut s = con_stato(rep(false, OverallStatus::Unprotected, "5.5.5.5"));
        // VPN accesa senza caduta precedente: nessun "ripristino"
        assert!(s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 0).is_empty());
        // Caduta
        let v = s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 100);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnCaduta]);
        // Ancora spenta: niente
        assert!(s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 200).is_empty());
        // Torna accesa
        let v = s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 300);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnRipristinata]);
        // Il ripristino e` notificato una volta sola
        assert!(s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 400).is_empty());
    }

    #[test]
    fn ripristino_resta_in_memoria_anche_se_la_caduta_era_stata_soppressa() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 0); // caduta notificata
        s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 60); // ripristino notificato
        // Seconda caduta e ripristino entro 30 min: la memoria funziona ma l'antiripetizione zittisce
        let v = s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 120);
        assert!(v.is_empty(), "caduta ripetuta entro 30 minuti");
        let v = s.controllo(&rep(true, OverallStatus::Protected, "9.9.9.9"), 180);
        assert!(v.is_empty(), "ripristino ripetuto entro 30 minuti");
        // Dopo 30 minuti dall'ultima notifica la caduta torna a essere notificata
        let v = s.controllo(&rep(false, OverallStatus::Unprotected, "5.5.5.5"), 3600);
        assert_eq!(tipi(&v), vec![TipoAvviso::VpnCaduta]);
    }

    #[test]
    fn antiripetizione_30_minuti_per_tipo() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut leak = rep(true, OverallStatus::Protected, "9.9.9.9");
        leak.dns_leak_detected = true;
        let pulito = rep(true, OverallStatus::Protected, "9.9.9.9");

        assert_eq!(tipi(&s.controllo(&leak, 0)), vec![TipoAvviso::LeakDns]);
        s.controllo(&pulito, 60);
        // Stessa transizione dopo 10 minuti: soppressa
        assert!(s.controllo(&leak, 600).is_empty());
        s.controllo(&pulito, 900);
        // Appena prima dei 30 minuti dalla prima notifica: ancora soppressa
        assert!(s.controllo(&leak, FINESTRA_ANTIRIPETIZIONE_SEC - 1).is_empty());
        s.controllo(&pulito, FINESTRA_ANTIRIPETIZIONE_SEC);
        // Dopo 30 minuti: di nuovo notificata
        assert_eq!(
            tipi(&s.controllo(&leak, FINESTRA_ANTIRIPETIZIONE_SEC + 1)),
            vec![TipoAvviso::LeakDns]
        );
    }

    #[test]
    fn tipi_diversi_non_si_zittiscono_a_vicenda() {
        let mut s = con_stato(rep(true, OverallStatus::Protected, "9.9.9.9"));
        let mut c = rep(true, OverallStatus::Warning, "9.9.9.9");
        c.dns_leak_detected = true;
        c.webrtc_leak_detected = true;
        let v = s.controllo(&c, 0);
        assert_eq!(
            tipi(&v),
            vec![TipoAvviso::VerdettoPeggiora, TipoAvviso::LeakDns, TipoAvviso::LeakWebrtc]
        );
    }

    #[test]
    fn download_sotto_soglia_solo_alla_transizione() {
        let soglie = Soglie { download_min_mbps: Some(50.0), ping_max_ms: None };
        let mut s = StatoNotifiche::default();
        // Prima misura gia` sotto soglia, senza precedente: avvisa
        let v = s.misura(&misura(30.0, 10.0), soglie, 0);
        assert_eq!(tipi(&v), vec![TipoAvviso::DownloadBasso]);
        // Ancora sotto soglia: nessuna transizione
        assert!(s.misura(&misura(20.0, 10.0), soglie, 5000).is_empty());
        // Risale e ricade (dopo 30 min): avvisa
        assert!(s.misura(&misura(80.0, 10.0), soglie, 6000).is_empty());
        let v = s.misura(&misura(10.0, 10.0), soglie, 9000);
        assert_eq!(tipi(&v), vec![TipoAvviso::DownloadBasso]);
        assert!(v[0].testo.contains("10.0 Mbps") && v[0].testo.contains("50.0"));
    }

    #[test]
    fn ping_sopra_soglia_e_soglie_spente() {
        let soglie = Soglie { download_min_mbps: None, ping_max_ms: Some(100.0) };
        let mut s = StatoNotifiche::default();
        assert!(s.misura(&misura(1.0, 50.0), soglie, 0).is_empty());
        let v = s.misura(&misura(1.0, 250.0), soglie, 10);
        assert_eq!(tipi(&v), vec![TipoAvviso::PingAlto]);
        // Soglie spente: mai avvisi
        let mut s2 = StatoNotifiche::default();
        assert!(s2.misura(&misura(0.1, 9999.0), Soglie::default(), 0).is_empty());
    }

    #[test]
    fn antiripetizione_vale_anche_per_la_velocita() {
        let soglie = Soglie { download_min_mbps: Some(50.0), ping_max_ms: None };
        let mut s = StatoNotifiche::default();
        assert_eq!(tipi(&s.misura(&misura(10.0, 1.0), soglie, 0)).len(), 1);
        s.misura(&misura(90.0, 1.0), soglie, 60);
        assert!(s.misura(&misura(10.0, 1.0), soglie, 120).is_empty(), "entro 30 minuti");
    }
}
