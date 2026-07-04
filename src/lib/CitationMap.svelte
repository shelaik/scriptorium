<script lang="ts">
  import type { SearchResult } from "$lib/api";

  // Mappa "a due ali" delle citazioni: a sinistra i riferimenti (il passato su
  // cui il paper si fonda), a destra chi lo cita (il futuro), ordinati per anno
  // dall'alto. Pallino pieno = già in libreria; tratteggiato = mancante; il
  // raggio cresce col numero di citazioni globali (OpenAlex). SVG puro, niente
  // dipendenze; i colori vengono dai token dell'app, quindi segue tutti i temi.
  let {
    refs,
    cits,
    title,
    onNode,
  }: {
    refs: SearchResult[];
    cits: SearchResult[];
    title: string;
    onNode: (r: SearchResult, e: MouseEvent) => void;
  } = $props();

  const MAX_PER_WING = 40;
  const ROW_H = 30;
  const PAD_TOP = 34;
  const PAD_BOTTOM = 18;
  const W = 1000; // viewBox width; scales to the container
  const X_REF = 330;
  const X_CIT = 670;
  const X_CENTER = 500;

  function wing(list: SearchResult[]): SearchResult[] {
    return [...list]
      .sort((a, b) => (a.year ?? 9999) - (b.year ?? 9999))
      .slice(0, MAX_PER_WING);
  }
  const left = $derived(wing(refs));
  const right = $derived(wing(cits));
  const rows = $derived(Math.max(left.length, right.length, 1));
  const H = $derived(PAD_TOP + rows * ROW_H + PAD_BOTTOM);
  const cy = $derived(PAD_TOP + (rows * ROW_H) / 2);

  const yAt = (i: number) => PAD_TOP + i * ROW_H + ROW_H / 2;
  /** Raggio del nodo: cresce (dolcemente) con le citazioni globali del paper. */
  const radius = (r: SearchResult) => 4 + Math.min(8, Math.log2((r.citations ?? 0) + 1) * 1.5);
  const short = (t: string | null, n = 40) => {
    const s = t ?? "Senza titolo";
    return s.length > n ? s.slice(0, n - 1) + "…" : s;
  };
  /** Arco morbido dal centro al nodo dell'ala. */
  function edge(x: number, y: number): string {
    const mx = (X_CENTER + x) / 2;
    return `M ${X_CENTER} ${cy} C ${mx} ${cy}, ${mx} ${y}, ${x} ${y}`;
  }
  function tip(r: SearchResult): string {
    const meta = [r.authors?.[0], r.year, r.venue].filter(Boolean).join(" · ");
    const cit = r.citations ? ` · ${r.citations} citazioni` : "";
    const lib = r.in_library ? "\n✓ già in libreria" : "\n○ non in libreria — clicca per aggiungerla";
    return `${r.title ?? "Senza titolo"}\n${meta}${cit}${lib}`;
  }
</script>

<div class="mapscroll">
  <svg viewBox="0 0 {W} {H}" style="min-height: {Math.min(H, 560)}px" role="img" aria-label="Mappa delle citazioni di {title}">
    <!-- ali -->
    {#each [{ items: left, x: X_REF, side: "l" }, { items: right, x: X_CIT, side: "r" }] as w (w.side)}
      {#each w.items as r, i (r.external_id)}
        {@const y = yAt(i)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <g
          class="node"
          class:inlib={r.in_library}
          role="button"
          tabindex="-1"
          onclick={(e) => onNode(r, e)}
        >
          <title>{tip(r)}</title>
          <path class="edge" d={edge(w.x, y)} />
          <circle class="dot" cx={w.x} cy={y} r={radius(r)} />
          {#if w.side === "l"}
            <text class="lbl" x={w.x - radius(r) - 8} y={y + 4} text-anchor="end">{short(r.title)}</text>
            <text class="yr" x={w.x - radius(r) - 8} y={y + 15} text-anchor="end">{r.year ?? ""}</text>
          {:else}
            <text class="lbl" x={w.x + radius(r) + 8} y={y + 4} text-anchor="start">{short(r.title)}</text>
            <text class="yr" x={w.x + radius(r) + 8} y={y + 15} text-anchor="start">{r.year ?? ""}</text>
          {/if}
        </g>
      {/each}
    {/each}

    <!-- centro: il paper esplorato -->
    <g class="center">
      <circle cx={X_CENTER} cy={cy} r="13" class="cdot" />
      <circle cx={X_CENTER} cy={cy} r="5" class="cdot inner" />
      <text class="ctitle" x={X_CENTER} y={cy - 22} text-anchor="middle">{short(title, 52)}</text>
    </g>

    <!-- intestazioni delle ali -->
    <text class="wingh" x={X_REF} y="16" text-anchor="end">← si fonda su</text>
    <text class="wingh" x={X_CIT} y="16" text-anchor="start">è citato da →</text>
  </svg>
  {#if refs.length > MAX_PER_WING || cits.length > MAX_PER_WING}
    <p class="mapnote">
      Mostro i primi {MAX_PER_WING} per lato ({refs.length} riferimenti, {cits.length} citazioni in tutto — la Lista li ha tutti).
    </p>
  {/if}
</div>

<style>
  .mapscroll { max-height: 56vh; overflow: auto; border: 1px solid var(--border-soft); border-radius: var(--r-md); background: var(--bg); }
  svg { display: block; width: 100%; height: auto; }

  .node { cursor: pointer; }
  .edge { fill: none; stroke: var(--border); stroke-width: 1.1; opacity: 0.7; transition: stroke 0.12s, opacity 0.12s; }
  .dot { fill: none; stroke: var(--dim); stroke-width: 1.4; stroke-dasharray: 3 3; transition: fill 0.12s, stroke 0.12s; }
  .node.inlib .dot { fill: var(--accent); stroke: var(--accent); stroke-dasharray: none; }
  .lbl { font-size: 11.5px; fill: var(--text); }
  .yr { font-size: 9px; fill: var(--faint); }
  .node:hover .edge, .node:focus .edge { stroke: var(--accent); opacity: 1; stroke-width: 1.8; }
  .node:hover .dot { stroke: var(--accent); }
  .node:hover .lbl { fill: var(--accent); font-weight: 600; }

  .cdot { fill: var(--accent-soft); stroke: var(--accent); stroke-width: 2; }
  .cdot.inner { fill: var(--accent); stroke: none; }
  .ctitle { font-size: 12.5px; font-weight: 600; fill: var(--text); }
  .wingh { font-size: 10.5px; font-weight: 700; letter-spacing: 0.5px; fill: var(--faint); text-transform: uppercase; }

  .mapnote { margin: 0; padding: 6px 12px; font-size: 11px; color: var(--faint); border-top: 1px solid var(--border-soft); }
</style>
