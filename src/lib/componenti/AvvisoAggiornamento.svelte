<script lang="ts">
  import { dashboard as d } from "$lib/stato.svelte";
  import { t } from "$lib/i18n/i18n.svelte";

  let agg = $derived(d.aggiornamento);
  let inCorso = $derived(d.statoAggiornamento === "download" || d.statoAggiornamento === "installazione");
  let visibile = $derived(agg !== null && (!d.avvisoNascosto || inCorso));
</script>

{#if agg && visibile}
  <section class="avviso-agg" aria-labelledby="titolo-agg">
    <div class="testa">
      <strong id="titolo-agg">{t("agg.titolo", { versione: agg.version })}</strong>
      {#if d.versioneApp}<span class="tenue">{t("agg.tua", { versione: d.versioneApp })}</span>{/if}
    </div>
    {#if agg.body}
      <div class="note">
        <span class="tenue">{t("agg.note")}</span>
        <pre>{agg.body}</pre>
      </div>
    {/if}
    {#if inCorso}
      <p class="stato" role="status">
        {#if d.statoAggiornamento === "installazione"}
          {t("agg.installazione")}
        {:else if d.percentualeDownload !== null}
          {t("agg.downloadPerc", { n: d.percentualeDownload })}
        {:else}
          {t("agg.download")}
        {/if}
      </p>
    {:else}
      <div class="azioni">
        <button onclick={() => d.rimandaAggiornamento()}>{t("agg.piuTardi")}</button>
        <button
          class="si"
          onclick={() => d.installaAggiornamento()}
          disabled={d.occupato}
          title={d.occupato ? t("agg.occupato") : undefined}
        >
          {t("agg.aggiornaOra")}
        </button>
      </div>
    {/if}
  </section>
{/if}

<style>
  .avviso-agg {
    position: fixed;
    top: 72px;
    right: 24px;
    z-index: 35;
    width: min(380px, calc(100vw - 32px));
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 16px;
    background: var(--superficie);
    border: 1px solid var(--accento);
    border-radius: var(--raggio);
    box-shadow: 0 8px 28px rgb(0 0 0 / 0.28);
  }
  .testa {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .tenue {
    color: var(--testo-tenue);
    font-size: 12px;
  }
  .note pre {
    margin: 4px 0 0;
    max-height: 140px;
    overflow-y: auto;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font: inherit;
    font-size: 13px;
    line-height: 1.45;
  }
  .stato {
    margin: 0;
    font-size: 13px;
    color: var(--testo-tenue);
  }
  .azioni {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  button {
    border-radius: 8px;
    padding: 7px 16px;
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    font-weight: 550;
  }
  .si {
    background: var(--accento);
    color: var(--accento-testo);
    border-color: var(--accento);
  }
  button:disabled {
    opacity: 0.55;
  }
</style>
