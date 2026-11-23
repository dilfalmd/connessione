<script lang="ts">
  import { dashboard as d } from "$lib/stato.svelte";
  import { numero } from "$lib/formati";
  import { t } from "$lib/i18n/i18n.svelte";
  import type { GruppoVpn } from "$lib/tipi";

  const MINIMO = 3;

  const vuoto = (vpn: boolean | null): GruppoVpn => ({
    vpnAttiva: vpn,
    misure: 0,
    download: { media: null, mediana: null },
    upload: { media: null, mediana: null },
    ping: { media: null, mediana: null },
  });

  let acceso = $derived(d.confronto.find((g) => g.vpnAttiva === true) ?? vuoto(true));
  let spento = $derived(d.confronto.find((g) => g.vpnAttiva === false) ?? vuoto(false));
  let sconosciute = $derived(d.confronto.find((g) => g.vpnAttiva === null)?.misure ?? 0);

  const nomeIntervallo = { "7": "intervallo.7", "30": "intervallo.30", "90": "intervallo.90", tutto: "intervallo.tutto" } as const;

  const righe = [
    { nome: "conf.download", campo: "download", cifre: 1 },
    { nome: "conf.upload", campo: "upload", cifre: 1 },
    { nome: "conf.ping", campo: "ping", cifre: 1 },
  ] as const;
</script>

<section class="card" aria-labelledby="titolo-confronto">
  <h2 id="titolo-confronto">{t("conf.titolo")}</h2>
  <p class="tenue sotto">{t("conf.intervallo", { nome: t(nomeIntervallo[d.intervallo]) })}</p>

  {#if d.storico.length === 0}
    <p class="vuoto">{t("conf.vuoto")}</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th></th>
          <th>{t("conf.vpnAccesa")}</th>
          <th>{t("conf.vpnSpenta")}</th>
        </tr>
      </thead>
      <tbody>
        <tr class="conteggio">
          <th scope="row">{t("conf.misure")}</th>
          <td class="num">{acceso.misure}</td>
          <td class="num">{spento.misure}</td>
        </tr>
        {#each righe as r (r.campo)}
          <tr>
            <th scope="row">{t(r.nome)}</th>
            {#each [acceso, spento] as g, i (i)}
              {#if g.misure < MINIMO}
                <td class="insufficiente">{t("conf.insufficiente")}</td>
              {:else}
                <td class="num">
                  <span class="valore">{numero(g[r.campo].media, r.cifre)}</span>
                  <span class="mediana">{t("conf.mediana", { v: numero(g[r.campo].mediana, r.cifre) })}</span>
                </td>
              {/if}
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="nota tenue">
      {t("conf.nota", { min: MINIMO })}
      {#if sconosciute > 0}{t("conf.sconosciute", { n: sconosciute })}{/if}
    </p>
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
    margin: 0;
    font-size: 15px;
    font-weight: 650;
  }
  .sotto {
    margin: 2px 0 10px;
    font-size: 12.5px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    padding: 8px 6px;
    text-align: right;
    border-bottom: 1px solid var(--bordo);
    vertical-align: top;
  }
  th:first-child {
    text-align: left;
    font-weight: 500;
    color: var(--testo-tenue);
  }
  thead th {
    font-weight: 650;
    color: var(--testo);
    border-bottom-width: 2px;
  }
  .valore {
    display: block;
    font-weight: 650;
    font-size: 16px;
  }
  .mediana {
    display: block;
    color: var(--testo-tenue);
    font-size: 12px;
  }
  .insufficiente {
    color: var(--testo-tenue);
    font-style: italic;
    font-size: 12.5px;
    vertical-align: middle;
  }
  .nota {
    font-size: 12px;
    margin: 10px 0 0;
  }
  .vuoto {
    color: var(--testo-tenue);
    text-align: center;
    padding: 20px 0;
  }
</style>
