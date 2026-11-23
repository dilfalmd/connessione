<script lang="ts">
  import { dashboard as d } from "$lib/stato.svelte";
  import { classeVerdetto, etichettaVerdetto, numero } from "$lib/formati";
  import { t } from "$lib/i18n/i18n.svelte";

  let r = $derived(d.report);
  let inCorso = $derived(d.fase === "sicurezza");

  const si = (v: boolean) => (v ? t("comune.si") : t("comune.no"));

  let tunnel = $derived(r ? r.activeTunnels.map((t) => t.name).join(", ") : "");
  let voci = $derived(r ? Object.entries(r.blacklistResults) : []);
  let verificate = $derived(voci.filter(([, v]) => v.stato !== "non_verificabile").length);
  let nonVerificabili = $derived(voci.length - verificate);
  let notaPbl = $derived(
    voci.find(([, v]) => v.stato === "pulito" && v.nota && v.nota.includes("PBL")),
  );

  let copiato = $state(false);
  async function copia() {
    if (!r?.ipv4) return;
    if (await d.copiaIp(r.ipv4)) {
      copiato = true;
      setTimeout(() => (copiato = false), 1600);
    }
  }
</script>

<section class="card" aria-labelledby="titolo-sicurezza">
  <div class="intestazione">
    <h2 id="titolo-sicurezza">{t("sic.titolo")}</h2>
    {#if inCorso}<span class="tenue piccolo">{t("sic.inCorso")}</span>{/if}
  </div>

  {#if r}
    <div class="verdetto">
      <span class="badge {classeVerdetto[r.overallStatus]}">{etichettaVerdetto(r.overallStatus)}</span>
      <span class="punteggio num"><strong>{r.securityScore}</strong><span class="tenue"> / 100</span></span>
    </div>
    <p class="riepilogo tenue">{r.overallSummary}</p>

    <dl class="dati">
      <dt>IPv4</dt>
      <dd class="con-azione">
        <span class="num">{r.ipv4 ?? t("comune.nd")}</span>
        {#if r.ipv4}
          <button class="copia" onclick={copia}>{copiato ? t("sic.copiato") : t("sic.copia")}</button>
        {/if}
      </dd>
      <dt>IPv6</dt><dd class="num">{r.ipv6 ?? t("comune.nessuno")}</dd>
      <dt>ISP</dt><dd>{r.isp}</dd>
      <dt>{t("sic.posizione")}</dt><dd>{[r.city, r.country].filter(Boolean).join(", ") || t("comune.nd")}</dd>
      <dt>{t("sic.vpnTunnel")}</dt>
      <dd>
        {r.vpnSconosciuta ? t("comune.sconosciuto") : si(r.isVpnOrProxyIp)}{tunnel
          ? ` - ${tunnel}`
          : r.isVpnOrProxyIp
            ? t("sic.ipDiVpn")
            : ""}
      </dd>
      <dt>{t("sic.leakDns")}</dt>
      <dd class={r.dnsLeakDetected ? "pericolo" : r.ednsLeakDetected ? "avviso" : "ok"}>
        {r.dnsLeakDetected ? t("sic.rilevato") : r.ednsLeakDetected ? t("sic.avvisoEdns") : t("sic.nessuno")}
      </dd>
      <dt>{t("sic.leakWebrtc")}</dt>
      <dd class={r.webrtcLeakDetected ? "pericolo" : "ok"}>
        {r.webrtcLeakDetected ? t("sic.rilevato") : t("sic.nessuno")}
      </dd>
      <dt>{t("sic.blacklist")}</dt>
      <dd>
        <span class={r.blacklistedCount > 0 ? "pericolo" : ""}>
          {t("sic.inListaSu", { n: r.blacklistedCount, v: verificate })}
        </span>
        {#if nonVerificabili > 0}<span class="nota">{t("sic.nonVerificabili", { n: nonVerificabili })}</span>{/if}
        {#if notaPbl}
          <div class="nota">{t("sic.notaPbl", { nome: notaPbl[0] })}</div>
        {/if}
      </dd>
    </dl>

    {#if r.overallStatus === "ERROR"}
      <p class="messaggio-errore">{r.overallSummary}</p>
    {/if}

    <details>
      <summary>{t("sic.dettagli")}</summary>
      <div class="dettagli">
        <h3>{t("sic.serverDns")}</h3>
        <ul>
          {#each r.dnsResolvers as s (s.ip)}
            <li>
              <span class="num">{s.ip}</span> - {s.geoOrg}
              {#if s.ednsIp}<span class="avviso"> (EDNS: {s.ednsIp})</span>{/if}
            </li>
          {:else}
            <li class="tenue">{t("sic.nessunResolver")}</li>
          {/each}
        </ul>
        <p class="nota">{t("sic.dnsSistema", { v: r.localDnsServers.join(", ") || t("comune.nd") })}</p>

        <h3>{t("sic.interfacce")}</h3>
        <ul>
          {#each r.allUpInterfaces as i (i.name)}
            <li>
              {i.name} - <span class="num">{i.ipv4}</span>, MTU {i.mtu}
              {#if i.isTunnel}<span class="etichetta">{t("sic.tunnel")}</span>{/if}
            </li>
          {:else}
            <li class="tenue">{t("sic.nessunaInterfaccia")}</li>
          {/each}
        </ul>
        <p class="nota">
          {t("sic.rotta", {
            a: si(r.defaultRouteViaTunnel),
            b: si(r.killswitchOrStrictRouteDetected),
            c: r.latencyMs === null ? t("comune.nd") : `${numero(r.latencyMs)} ms`,
          })}
        </p>

        <h3>{t("sic.blacklist")}</h3>
        <ul>
          {#each voci as [nome, v] (nome)}
            <li>
              {nome}:
              <span class={v.stato === "in_lista" ? "pericolo" : v.stato === "pulito" ? "ok" : "avviso"}>
                {t(`dnsbl.${v.stato}`)}
              </span>
              {#if v.nota}<span class="nota"> - {v.nota}</span>{/if}
            </li>
          {/each}
        </ul>

        {#if r.warnings.length}
          <h3>{t("sic.avvisi")}</h3>
          <ul>{#each r.warnings as w, i (i)}<li class="avviso">{w}</li>{/each}</ul>
        {/if}
        {#if r.positives.length}
          <h3>{t("sic.positivi")}</h3>
          <ul>{#each r.positives as p, i (i)}<li class="ok">{p}</li>{/each}</ul>
        {/if}
      </div>
    </details>
  {:else if d.caricamentoIniziale || inCorso}
    <p class="vuoto">{t("sic.verifica")}</p>
  {:else}
    <p class="vuoto">{t("sic.nessunControllo")}</p>
  {/if}

  {#if d.erroreSicurezza}
    <p class="messaggio-errore">{t("sic.controlloFallito", { err: d.erroreSicurezza })}</p>
  {/if}
</section>

<style>
  .card {
    background: var(--superficie);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio);
    box-shadow: var(--ombra);
    padding: 18px 20px;
    min-width: 0;
  }
  .intestazione {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }
  h2 {
    margin: 0 0 10px;
    font-size: 15px;
    font-weight: 650;
  }
  h3 {
    margin: 12px 0 4px;
    font-size: 12.5px;
    color: var(--testo-tenue);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .piccolo {
    font-size: 12px;
  }
  .verdetto {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .badge {
    padding: 4px 14px;
    border-radius: 999px;
    font-weight: 650;
    font-size: 15px;
  }
  .badge.ok { background: var(--ok-sfondo); color: var(--ok); }
  .badge.avviso { background: var(--avviso-sfondo); color: var(--avviso); }
  .badge.pericolo { background: var(--pericolo-sfondo); color: var(--pericolo); }
  .badge.neutro { background: var(--neutro-sfondo); color: var(--neutro); }
  .punteggio strong {
    font-size: 26px;
    font-weight: 650;
  }
  .riepilogo {
    margin: 6px 0 12px;
    font-size: 13px;
  }
  .dati {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  dt {
    color: var(--testo-tenue);
  }
  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .con-azione {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .copia {
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    border-radius: 6px;
    padding: 1px 9px;
    font-size: 12px;
  }
  .copia:hover {
    border-color: var(--accento);
  }
  .ok { color: var(--ok); }
  .avviso { color: var(--avviso); }
  .pericolo { color: var(--pericolo); }
  .nota {
    color: var(--testo-tenue);
    font-size: 12.5px;
  }
  details {
    margin-top: 14px;
    border-top: 1px solid var(--bordo);
    padding-top: 10px;
  }
  summary {
    cursor: pointer;
    font-weight: 600;
  }
  .dettagli ul {
    margin: 0;
    padding-left: 18px;
  }
  .etichetta {
    font-size: 11px;
    border: 1px solid var(--bordo);
    border-radius: 4px;
    padding: 0 5px;
    color: var(--testo-tenue);
  }
  .vuoto {
    color: var(--testo-tenue);
    padding: 24px 0;
    text-align: center;
  }
  .messaggio-errore {
    margin: 10px 0 0;
    padding: 8px 12px;
    border-radius: 8px;
    background: var(--pericolo-sfondo);
    color: var(--pericolo);
  }
</style>
