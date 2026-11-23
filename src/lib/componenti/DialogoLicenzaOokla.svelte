<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { dashboard as d } from "$lib/stato.svelte";
  import { t } from "$lib/i18n/i18n.svelte";

  const collegamenti = [
    { testo: "lic.eula", url: "https://www.speedtest.net/about/eula" },
    { testo: "lic.termini", url: "https://www.speedtest.net/about/terms" },
    { testo: "lic.privacy", url: "https://www.speedtest.net/about/privacy" },
  ] as const;

  let dialogo = $derived(d.licenza);
  let erroreLink = $state("");

  async function apri(e: MouseEvent, url: string) {
    e.preventDefault();
    erroreLink = "";
    try {
      await openUrl(url);
    } catch {
      erroreLink = t("lic.erroreLink", { url });
    }
  }

  function tasto(e: KeyboardEvent) {
    if (dialogo && e.key === "Escape") dialogo.risolvi(false);
  }
</script>

<svelte:window onkeydown={tasto} />

{#if dialogo}
  <div class="velo" role="presentation">
    <div class="finestra" role="alertdialog" aria-modal="true" aria-labelledby="testo-licenza">
      <p id="testo-licenza">
        {t("lic.testo")}
      </p>
      <ul>
        {#each collegamenti as c (c.url)}
          <li><a href={c.url} onclick={(e) => apri(e, c.url)}>{t(c.testo)}</a></li>
        {/each}
      </ul>
      {#if erroreLink}<p class="errore">{erroreLink}</p>{/if}
      <div class="azioni">
        <button onclick={() => dialogo?.risolvi(false)}>{t("lic.annulla")}</button>
        <!-- svelte-ignore a11y_autofocus -->
        <button class="si" autofocus onclick={() => dialogo?.risolvi(true)}>{t("lic.accetto")}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .velo {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(8 12 20 / 0.5);
  }
  .finestra {
    width: min(460px, calc(100vw - 32px));
    background: var(--superficie);
    border: 1px solid var(--bordo);
    border-radius: var(--raggio);
    padding: 22px;
    box-shadow: 0 12px 40px rgb(0 0 0 / 0.35);
  }
  p {
    margin: 0 0 12px;
    font-size: 15px;
    line-height: 1.5;
  }
  ul {
    margin: 0 0 18px;
    padding-left: 20px;
    line-height: 1.7;
  }
  a {
    color: var(--accento);
    cursor: pointer;
  }
  .errore {
    font-size: 13px;
    color: var(--pericolo);
  }
  .azioni {
    display: flex;
    justify-content: flex-end;
    gap: 10px;
  }
  button {
    border-radius: 8px;
    padding: 8px 18px;
    border: 1px solid var(--bordo);
    background: var(--superficie-2);
    font-weight: 550;
  }
  .si {
    background: var(--accento);
    color: var(--accento-testo);
    border-color: var(--accento);
  }
</style>
