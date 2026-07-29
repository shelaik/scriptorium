<script lang="ts">
  /* Guscio della prosa della guida.
   *
   * Due responsabilita', entrambe strutturali:
   *
   *  1. SCEGLIE LA LINGUA. La guida e' ~4.800 parole per lingua: nel dizionario
   *     diventerebbe illeggibile, e ritoccare una virgola in italiano orfanerebbe
   *     in silenzio l'intera traduzione. Quindi due componenti gemelli di solo
   *     markup, montati in base a `i18n.locale` (getter su $state: cambiare
   *     lingua rimonta la prosa senza ricaricare — al prezzo, accettabile, di
   *     riportare in cima lo scorrimento di .helpbody).
   *
   *  2. POSSIEDE IL CSS. Il foglio di +page.svelte e' scoped: le regole
   *     `.helpsec`, `.faq`, `.kbdtable` smettono di corrispondere appena il
   *     markup esce da quel file. Sono riscritte qui ancorate a `.helpprose`,
   *     che sta nel markup di QUESTO componente, con `:global(…)` sui figli.
   *     Niente `:global { … }` a blocco: `.faq` e' un nome troppo comune per
   *     finire nel foglio globale.
   *
   * `.helpwin kbd` resta invece in +page.svelte, riscritta `.helpwin :global(kbd)`:
   * l'antenato e' markup della pagina e cosi' copre in un colpo solo tutti i
   * tasti della finestra.
   */
  import { i18n } from "$lib/i18n/index.svelte";
  import type { HelpTab } from "./tabs";
  import HelpIt from "./HelpIt.svelte";
  import HelpEn from "./HelpEn.svelte";

  let { tab }: { tab: HelpTab } = $props();
</script>

<div class="helpprose">
  {#if i18n.locale === "en"}<HelpEn {tab} />{:else}<HelpIt {tab} />{/if}
</div>

<style>
  /* L'occhiello d'apertura resta in +page.svelte (sta SOPRA le linguette, e
     spostarlo qui lo avrebbe portato sotto): la sua regola `.helpbody > .dimtext`
     resta li'. Il .dimtext annidato dentro «Inizia qui» non aveva una regola che
     lo raggiungesse e continua a non averla. */
  .helpprose :global(.faq) { margin: 0; }
  .helpprose :global(.faq dt) { font-size: 13px; font-weight: 600; color: var(--text); margin-top: 10px; }
  .helpprose :global(.faq dd) { margin: 2px 0 0; font-size: 13px; line-height: 1.5; color: var(--dim); }
  .helpprose :global(.helpsec) { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--border); }
  .helpprose :global(.helpsec h3) { font-size: 14px; font-family: var(--serif); margin: 0 0 8px; color: var(--text); }
  .helpprose :global(.helpsec ul) { margin: 0; padding-left: 18px; }
  .helpprose :global(.helpsec li) { font-size: 13px; line-height: 1.55; color: var(--text); margin: 4px 0; }
  .helpprose :global(.kbdtable) { width: 100%; border-collapse: collapse; margin-top: 10px; }
  .helpprose :global(.kbdtable td) { padding: 4px 8px; font-size: 12.5px; border-bottom: 1px solid var(--border-soft); color: var(--dim); }
  .helpprose :global(.kbdtable td:first-child) { white-space: nowrap; width: 1%; }
</style>
