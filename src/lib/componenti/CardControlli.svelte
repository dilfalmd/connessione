<script lang="ts">
  import { dashboard as d } from "$lib/stato.svelte";
  import { classeVerdetto, dataOra, etichettaVerdetto } from "$lib/formati";
  import { t } from "$lib/i18n/i18n.svelte";

  let sel = $derived(d.controlloSelezionato);
  let r = $derived(sel && sel.report && typeof sel.report === "object" ? sel.report : null);
  let verificateSel = $derived(
    r?.blacklistResults
      ? Object.values(r.blacklistResults).filter((v) => v.stato !== "non_verificabile").length
      : 0,
  );

  const statoVpn = (v: boolean | null) =>
    v === true ? t("comune.si") : v === false ? t("comune.no") : t("comune.sconosciuto");
  const sn = (v: boolean) => (v ? t("comune.siMin") : t("comune.noMin"));
</script>

<section class="card" aria-labelledby="titolo-controlli">
  <h2 id="titolo-controlli">{t("ctrl.titolo")}</h2>

  {#if d.controlli.length === 0}
    <p class="vuoto">
      {d.caricamentoIniziale ? t("comune.caricamento") : t("ctrl.nessuno")}
    </p>
  {:else}
    <table>
      <thead>
        <tr><th>{t("ctrl.data")}</th><th>{t("ctrl.verdetto")}</th><th>{t("ctrl.ip")}</th><th>{t("ctrl.vpn")}</th></tr>
      </thead>
      <tbody>
        {#each d.controlli as c (c.id)}
          <tr class:selezionata={sel?.id === c.id}>
            <td>
              <button class="riga" onclick={() => d.selezionaControllo(c)} aria-expanded={sel?.id === c.id}>
                {dataOra(c.timestamp)}
              </button>
            </td>
            <td class="verdetto {classeVerdetto[c.verdetto]}">
              {etichettaVerdetto(c.verdetto)} <span class="tenue num">({c.punteggio})</span>
            </td>
            <td class="num">{c.ipv4 ?? t("comune.nd")}</td>
            <td>{statoVpn(c.vpnAttiva)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if sel}
    <div class="dettaglio">
      <div class="testata">
        <h3>{t("ctrl.controlloDel", { data: dataOra(sel.timestamp) })}</h3>
        <button class="esporta" onclick={() => d.esportaReport(sel.id)}>{t("ctrl.esporta")}</button>
      </div>
      {#if r && r.overallTitle}
        <p class="titolo {classeVerdetto[sel.verdetto]}">{r.overallTitle}</p>
        <p class="tenue">{r.overallSummary}</p>
        <dl>
          <dt>ISP</dt><dd>{r.isp}</dd>
          <dt>{t("ctrl.posizione")}</dt><dd>{[r.city, r.country].filter(Boolean).join(", ")}</dd>
          <dt>{t("ctrl.leak")}</dt>
          <dd>
            {t("ctrl.leakRiga", {
              dns: sn(r.dnsLeakDetected),
              web: sn(r.webrtcLeakDetected),
              v6: sn(r.ipv6LeakDetected),
            })}
          </dd>
          {#if r.blacklistResults}
            <dt>{t("ctrl.blacklist")}</dt>
            <dd>{t("ctrl.inListaSu", { n: r.blacklistedCount, v: verificateSel })}</dd>
          {/if}
        </dl>
        {#if r.warnings?.length}
          <ul>{#each r.warnings as w, i (i)}<li>{w}</li>{/each}</ul>
        {/if}
      {:else}
        <p class="tenue">{t("ctrl.nonDisponibile")}</p>
      {/if}
    </div>
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
  h2 {
    margin: 0 0 8px;
    font-size: 15px;
    font-weight: 650;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 6px 8px;
    border-bottom: 1px solid var(--bordo);
  }
  th {
    font-size: 12px;
    font-weight: 600;
    color: var(--testo-tenue);
  }
  tr.selezionata td {
    background: var(--superficie-2);
  }
  .riga {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accento);
    text-align: left;
    font-variant-numeric: tabular-nums;
  }
  .riga:hover {
    text-decoration: underline;
  }
  .verdetto { font-weight: 600; }
  .ok { color: var(--ok); }
  .avviso { color: var(--avviso); }
  .pericolo { color: var(--pericolo); }
  .neutro { color: var(--neutro); }
  .vuoto {
    color: var(--testo-tenue);
    text-align: center;
    padding: 20px 0;
  }
  .dettaglio {
    margin-top: 14px;
    padding: 12px 14px;
    background: var(--superficie-2);
    border: 1px solid var(--bordo);
    border-radius: 10px;
  }
  .testata {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  h3 {
    margin: 0;
    font-size: 14px;
  }
  .esporta {
    border: 1px solid var(--accento);
    background: var(--accento);
    color: var(--accento-testo);
    border-radius: 8px;
    padding: 6px 14px;
    font-weight: 550;
  }
  .titolo {
    margin: 8px 0 2px;
    font-weight: 650;
  }
  p {
    margin: 4px 0;
  }
  dl {
    display: grid;
    grid-template-columns: 90px 1fr;
    gap: 4px 10px;
    margin: 8px 0;
  }
  dt { color: var(--testo-tenue); }
  dd { margin: 0; }
  ul {
    margin: 6px 0 0;
    padding-left: 18px;
    color: var(--avviso);
  }
</style>
