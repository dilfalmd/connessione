<script lang="ts">
  import { dashboard as d } from "$lib/stato.svelte";
  import { dataOra, numero } from "$lib/formati";
  import { t } from "$lib/i18n/i18n.svelte";

  const nomeFase = { ping: "vel.ping", download: "vel.download", upload: "vel.upload" } as const;
  const pesoFase = { ping: 0, download: 1, upload: 2 } as const;

  let m = $derived(d.ultimaMedia);
  let inTest = $derived(d.fase === "velocita");
  let inSicurezza = $derived(d.fase === "sicurezza");

  /** Avanzamento complessivo 0-1 su ripetizioni x (ping, download, upload) */
  let avanzamento = $derived(
    d.progresso
      ? (d.progresso.ripetizione - 1 +
          (pesoFase[d.progresso.fase] + d.progresso.avanzamento) / 3) /
          d.progresso.totale
      : 0,
  );
</script>

<section class="card" aria-labelledby="titolo-velocita">
  <h2 id="titolo-velocita">{t("vel.titolo")}</h2>

  {#if !d.cliPresente}
    <div class="vuoto">
      <p>{t("vel.cliMancante")}</p>
      <button class="azione" onclick={() => d.installaCli()} disabled={d.installazione === "in corso"}>
        {d.installazione === "in corso" ? t("vel.installazioneInCorso") : t("vel.installa")}
      </button>
      {#if d.installazione === "errore"}<p class="errore">{d.messaggioInstall}</p>{/if}
    </div>
  {:else if inTest}
    <div class="test">
      <p class="fase">
        {#if d.progresso}
          {t(nomeFase[d.progresso.fase])}
          <span class="num valore">
            {numero(d.progresso.valore)} {d.progresso.fase === "ping" ? "ms" : "Mbps"}
          </span>
        {:else}
          {t("vel.avvio")}
        {/if}
      </p>
      <p class="tenue">
        {#if d.progresso}{t("vel.ripetizioneDi", { n: d.progresso.ripetizione, tot: d.progresso.totale })}{:else}&nbsp;{/if}
      </p>
      <progress max="1" value={d.progresso ? avanzamento : undefined} aria-label={t("vel.avanzamento")}></progress>
      <button class="annulla" onclick={() => d.annulla()}>{t("vel.annulla")}</button>
    </div>
  {:else if inSicurezza && d.occupato && !m}
    <p class="vuoto">{t("vel.attesaSicurezza")}</p>
  {:else if m}
    <div class="grandi">
      <div>
        <span class="etichetta">{t("vel.download")}</span>
        <span class="grande num">{numero(m.downloadMbps)}<small> Mbps</small></span>
      </div>
      <div>
        <span class="etichetta">{t("vel.upload")}</span>
        <span class="grande num">{numero(m.uploadMbps)}<small> Mbps</small></span>
      </div>
    </div>
    <dl class="dati">
      <dt>{t("vel.ping")}</dt><dd class="num">{numero(m.pingMs)} ms</dd>
      <dt>{t("vel.jitter")}</dt><dd class="num">{m.jitterMs === null ? t("comune.nd") : `${numero(m.jitterMs)} ms`}</dd>
      <dt>{t("vel.packetLoss")}</dt>
      <dd class="num">{m.packetLoss === null ? t("comune.nd") : `${numero(m.packetLoss, 2)} %`}</dd>
      <dt>{t("vel.server")}</dt>
      <dd>{m.server ?? t("comune.nd")}{m.luogo ? ` (${m.luogo})` : ""}</dd>
      <dt>{t("vel.vpn")}</dt>
      <dd class="vpn">
        <svg width="14" height="14" viewBox="0 0 14 14" aria-hidden="true">
          {#if m.vpnAttiva === true}
            <circle cx="7" cy="7" r="5" fill="var(--ok)" />
          {:else if m.vpnAttiva === false}
            <circle cx="7" cy="7" r="4.2" fill="none" stroke="var(--testo-tenue)" stroke-width="1.8" />
          {:else}
            <circle cx="7" cy="7" r="5" fill="var(--neutro)" />
          {/if}
        </svg>
        {m.vpnAttiva === true ? t("vel.vpnAccesa") : m.vpnAttiva === false ? t("vel.vpnSpenta") : t("vel.vpnSconosciuta")}
      </dd>
      <dt>{t("vel.data")}</dt>
      <dd class="num">
        {dataOra(m.timestamp)}{#if m.n > 1}<span class="tenue">{t("vel.mediaDi", { n: m.n })}</span>{/if}
      </dd>
    </dl>
  {:else if d.caricamentoIniziale}
    <p class="vuoto">{t("vel.caricamentoStorico")}</p>
  {:else}
    <p class="vuoto">{t("vel.vuoto")}</p>
  {/if}

  {#if d.erroreVelocita}<p class="errore">{d.erroreVelocita}</p>{/if}
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
    margin: 0 0 10px;
    font-size: 15px;
    font-weight: 650;
  }
  .grandi {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-bottom: 14px;
  }
  .etichetta {
    display: block;
    font-size: 12px;
    color: var(--testo-tenue);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .grande {
    font-size: 38px;
    font-weight: 650;
    letter-spacing: -0.02em;
    line-height: 1.1;
  }
  .grande small {
    font-size: 14px;
    font-weight: 500;
    color: var(--testo-tenue);
    letter-spacing: 0;
  }
  .dati {
    display: grid;
    grid-template-columns: 100px 1fr;
    gap: 6px 12px;
    margin: 0;
  }
  dt { color: var(--testo-tenue); }
  dd { margin: 0; min-width: 0; overflow-wrap: anywhere; }
  .vpn { display: flex; align-items: center; gap: 6px; }
  .test {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 6px 0;
  }
  .fase {
    margin: 0;
    font-size: 20px;
    font-weight: 600;
  }
  .valore {
    margin-left: 8px;
    color: var(--accento);
  }
  progress {
    width: 100%;
    height: 10px;
    accent-color: var(--accento);
  }
  .test p { margin: 0; }
  .annulla, .azione {
    align-self: flex-start;
    margin-top: 6px;
    border-radius: 8px;
    padding: 7px 16px;
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    font-weight: 550;
  }
  .annulla:hover, .azione:hover:not(:disabled) { border-color: var(--pericolo); color: var(--pericolo); }
  .azione:hover:not(:disabled) { border-color: var(--accento); color: var(--accento); }
  .vuoto { color: var(--testo-tenue); padding: 16px 0; text-align: center; }
  .vuoto p { margin: 0 0 8px; }
  .vuoto .azione { align-self: center; }
  .errore {
    margin: 10px 0 0;
    padding: 8px 12px;
    border-radius: 8px;
    background: var(--pericolo-sfondo);
    color: var(--pericolo);
  }
</style>
