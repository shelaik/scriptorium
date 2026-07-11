<script module lang="ts">
  export type { RadialItem } from "$lib/radial";
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import type { RadialItem } from "$lib/radial";

  let {
    x,
    y,
    items,
    title,
    subtitle = "",
    thumb = null,
    onclose,
  }: {
    x: number;
    y: number;
    items: RadialItem[];
    title: string;
    subtitle?: string;
    thumb?: string | null;
    onclose: () => void;
  } = $props();

  // Un "petalo" visibile: voce + etichetta (con briciole in modalità filtro).
  type Petal = { item: RadialItem; label: string; trail: RadialItem[] };

  const HUB = 46; // raggio del mozzo (px): dimensione visiva e zona di hit coincidono
  const MARGIN = 14;
  const LABEL_W = 170; // deve combaciare con max-width di .label nel blocco di stile
  const MIN_ARC = 76; // distanza minima fra i centri di due petali adiacenti (i petali sono 54px)
  const LABEL_D = 41; // distanza base petalo→ancora dell'etichetta lungo il raggio (27 petalo + 14 aria)
  const FLARE = 34; // vicino ai poli l'etichetta esce PIÙ lontano lungo il raggio (svasatura)
  const LABEL_GAP = 32; // distacco verticale minimo fra i CENTRI di due etichette (~23px di scatola)
  const PAD_X = LABEL_D + LABEL_W + 8; // sporgenza orizzontale delle etichette oltre R
  const PAD_Y = 172; // sporgenza verticale: etichette svasate + chip titolo + barra hint adattiva

  let el = $state<HTMLDivElement | null>(null);
  let stack = $state.raw<RadialItem[][]>([]); // solo sotto-anelli; la radice è `items`
  let path = $state.raw<RadialItem[]>([]);
  let hi = $state(-1);
  let query = $state("");
  let ringId = $state(0); // cambia → l'anello rifiorisce
  let imploding = $state(false);
  let vw = $state(1280);
  let vh = $state(800);
  let lastMx: number | null = null;
  let lastMy: number | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;

  const reduced =
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  if (typeof window !== "undefined") {
    vw = window.innerWidth;
    vh = window.innerHeight;
  }

  // ----- albero appiattito per la modalità filtro -----
  const flatAll = $derived.by(() => {
    const out: Petal[] = [];
    const walk = (list: RadialItem[], trail: RadialItem[]) => {
      for (const it of list) {
        if (it.disabled) continue;
        out.push({
          item: it,
          trail,
          label: [...trail.map((t) => t.label), it.label].join(" · "),
        });
        if (it.children?.length) walk(it.children, [...trail, it]);
      }
    };
    walk(items, []);
    return out;
  });

  function norm(s: string): string {
    // strip combining diacritics (U+0300-U+036F) after NFD decomposition
    return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  }

  // Sottosequenza fuzzy: bonus inizio-parola e caratteri consecutivi.
  function fuzzyScore(q: string, target: string): number | null {
    const t = norm(target);
    let ti = 0;
    let prev = -2;
    let score = 0;
    for (const ch of norm(q)) {
      const found = t.indexOf(ch, ti);
      if (found < 0) return null;
      score += 1;
      if (found === 0 || !/[a-z0-9]/.test(t[found - 1])) score += 3;
      if (found === prev + 1) score += 2;
      prev = found;
      ti = found + 1;
    }
    return score - t.length * 0.01;
  }

  const matches = $derived.by(() => {
    if (!query) return [] as Petal[];
    const scored: { p: Petal; s: number }[] = [];
    for (const p of flatAll) {
      const s = fuzzyScore(query, p.label);
      if (s !== null) scored.push({ p, s });
    }
    scored.sort((a, b) => b.s - a.s);
    return scored.slice(0, 8).map((e) => e.p);
  });

  const current = $derived(stack[stack.length - 1] ?? items);
  const ring = $derived<Petal[]>(
    query ? matches : current.map((it) => ({ item: it, trail: path, label: it.label }))
  );
  // Il raggio cresce col numero di voci: i petali non si sovrappongono mai
  // (con ~20 voci l'anello passa da 156 a ~200px), con poche resta compatto.
  const R = $derived(
    Math.max(
      ring.length <= 6 ? 118 : ring.length <= 8 ? 136 : 156,
      Math.ceil((ring.length * MIN_ARC) / (2 * Math.PI)),
    ),
  );

  function clampC(v: number, lo: number, hiV: number): number {
    return hiV < lo ? (lo + hiV) / 2 : Math.max(lo, Math.min(hiV, v));
  }
  const cx = $derived(clampC(x, MARGIN + R + PAD_X, vw - MARGIN - R - PAD_X));
  const cy = $derived(clampC(y, MARGIN + R + PAD_Y, vh - MARGIN - R - PAD_Y));

  // Etichette a callout radiale: ogni etichetta è ancorata LUNGO il raggio del
  // suo petalo (vicino ai poli esce più lontano: svasatura FLARE, così le
  // etichette laterali partono oltre quella centrata del polo) e un raggio
  // disegnato collega il petalo all'etichetta. hem: r = ancorata al bordo
  // sinistro (cresce a destra), l = al bordo destro, c = centrata (poli).
  type Geo = {
    p: Petal;
    tx: number;
    ty: number;
    hem: "r" | "l" | "c";
    lx: number; // ancora etichetta (coordinate stage, dy già escluso)
    ly: number;
    dy: number; // scostamento verticale anti-collisione
    rx1: number; // raggio petalo→etichetta
    ry1: number;
    rx2: number;
    ry2: number;
  };
  const geo = $derived.by<Geo[]>(() => {
    const gs: Geo[] = ring.map((p, i) => {
      const rad = ((-90 + (i * 360) / Math.max(ring.length, 1)) * Math.PI) / 180;
      const cos = Math.cos(rad);
      const sin = Math.sin(rad);
      const d = R + LABEL_D + FLARE * sin * sin;
      return {
        p,
        tx: R * cos,
        ty: R * sin,
        hem: Math.abs(cos) <= 0.16 ? "c" : cos > 0 ? "r" : "l",
        lx: d * cos,
        ly: d * sin,
        dy: 0,
        rx1: (R + 28) * cos,
        ry1: (R + 28) * sin,
        rx2: 0,
        ry2: 0,
      };
    });
    // Anti-collisione (come nei grafici a torta): per lato, impila le ancore
    // dall'alto imponendo un distacco minimo, poi ricentra il gruppo.
    for (const side of ["r", "l"] as const) {
      const grp = gs.filter((g) => g.hem === side).sort((a, b) => a.ly - b.ly);
      if (grp.length < 2) continue;
      const pos: number[] = [];
      for (let i = 0; i < grp.length; i++) {
        pos.push(i === 0 ? grp[i].ly : Math.max(pos[i - 1] + LABEL_GAP, grp[i].ly));
      }
      const drift = pos.reduce((s, v, i) => s + (v - grp[i].ly), 0) / grp.length;
      for (let i = 0; i < grp.length; i++) grp[i].dy = pos[i] - drift - grp[i].ly;
    }
    // Estremo del raggio: si ferma poco prima dell'etichetta (che può essere
    // stata scostata di dy), così la linea "insegue" la sua etichetta.
    for (const g of gs) {
      const ex = g.lx;
      const ey = g.ly + g.dy;
      const ddx = ex - g.rx1;
      const ddy = ey - g.ry1;
      const len = Math.hypot(ddx, ddy) || 1;
      const pull = g.hem === "c" ? 16 : 6;
      g.rx2 = ex - (pull * ddx) / len;
      g.ry2 = ey - (pull * ddy) / len;
    }
    return gs;
  });

  const hintText = $derived(hi >= 0 ? (ring[hi]?.item.hint ?? "") : "");
  const announce = $derived(hi >= 0 ? (ring[hi]?.label ?? "") : "");
  const hubLetter = $derived(title.trim().charAt(0).toUpperCase() || "S");

  // ----- geometria puntatore → petalo -----
  function nearestEnabled(i: number): number {
    const n = ring.length;
    if (!n) return -1;
    for (let k = 0; k <= n; k++) {
      for (const c of k === 0 ? [i] : [(i + k) % n, (i - k + n * 2) % n]) {
        if (!ring[c]?.item.disabled) return c;
      }
    }
    return -1;
  }

  function hiFromPoint(px: number, py: number): number {
    const n = ring.length;
    if (!n) return -1;
    const dx = px - cx;
    const dy = py - cy;
    if (Math.hypot(dx, dy) <= HUB) return -1;
    const rel = ((Math.atan2(dy, dx) * 180) / Math.PI + 90 + 360) % 360;
    return nearestEnabled(Math.round(rel / (360 / n)) % n);
  }

  function stepHi(dir: 1 | -1) {
    const n = ring.length;
    if (!n) return;
    if (hi < 0) {
      hi = nearestEnabled(dir === 1 ? 0 : n - 1);
      return;
    }
    let j = hi;
    for (let k = 0; k < n; k++) {
      j = (j + dir + n) % n;
      if (!ring[j]?.item.disabled) {
        hi = j;
        return;
      }
    }
  }

  // ----- transizioni anello (implosione → nuova fioritura; il mozzo resta) -----
  function swapRing(fn: () => void) {
    if (imploding) return;
    const apply = () => {
      fn();
      ringId++;
      imploding = false;
      hi = lastMx === null || lastMy === null ? -1 : hiFromPoint(lastMx, lastMy);
    };
    if (reduced) {
      apply();
      return;
    }
    imploding = true;
    timer = setTimeout(apply, 110);
  }

  function pushRing(item: RadialItem) {
    swapRing(() => {
      stack = [...stack, item.children ?? []];
      path = [...path, item];
    });
  }

  function popRing() {
    swapRing(() => {
      stack = stack.slice(0, -1);
      path = path.slice(0, -1);
    });
  }

  // ----- attivazione -----
  function activate(idx: number = hi) {
    if (imploding) return;
    const p = ring[idx];
    if (!p || p.item.disabled) return;
    if (query) {
      activateFlat(p);
      return;
    }
    if (p.item.children?.length) pushRing(p.item);
    else {
      p.item.action?.();
      onclose();
    }
  }

  function activateFlat(p: Petal) {
    const it = p.item;
    if (it.children?.length) {
      const chain = [...p.trail, it];
      swapRing(() => {
        stack = chain.map((b) => b.children ?? []);
        path = chain;
        query = "";
      });
    } else {
      it.action?.();
      onclose();
    }
  }

  // ----- modalità filtro -----
  function appendQuery(ch: string) {
    const was = query;
    query = was + ch;
    if (!was) ringId++;
    hi = ring.length ? 0 : -1;
  }

  function clearQuery() {
    query = "";
    ringId++;
    hi = -1;
  }

  // ----- eventi -----
  function onMove(e: MouseEvent) {
    lastMx = e.clientX;
    lastMy = e.clientY;
    if (!imploding) hi = hiFromPoint(e.clientX, e.clientY);
  }

  function engage(px: number, py: number) {
    if (imploding) return;
    const d = Math.hypot(px - cx, py - cy);
    if (d <= HUB) {
      if (query) clearQuery();
      else if (stack.length > 0) popRing();
      else onclose();
      return;
    }
    if (d > R * 2.2) {
      onclose();
      return;
    }
    const idx = hiFromPoint(px, py);
    if (idx >= 0) activate(idx);
  }

  // Click sull'elemento petalo/etichetta (anche oltre R*2.2): attiva quel petalo.
  function petalFromTarget(t: EventTarget | null): number {
    if (!(t instanceof Element)) return -1;
    const w = t.closest("[data-petal]");
    if (!(w instanceof HTMLElement)) return -1;
    const n = Number(w.dataset.petal);
    return Number.isInteger(n) && n >= 0 && n < ring.length ? n : -1;
  }

  function onClick(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const pi = petalFromTarget(e.target);
    if (pi >= 0) activate(pi);
    else engage(e.clientX, e.clientY);
  }

  function onCtx(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    const pi = petalFromTarget(e.target);
    if (pi >= 0) activate(pi);
    else engage(e.clientX, e.clientY);
  }

  function onWheel(e: WheelEvent) {
    e.preventDefault();
    e.stopPropagation();
    stepHi(e.deltaY > 0 ? 1 : -1);
  }

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    const k = e.key;
    if (k === "ArrowRight" || k === "ArrowDown") {
      e.preventDefault();
      stepHi(1);
    } else if (k === "ArrowLeft" || k === "ArrowUp") {
      e.preventDefault();
      stepHi(-1);
    } else if (k === "Tab") {
      // trappola di focus: Tab non esce dall'overlay, ruota l'evidenziazione
      e.preventDefault();
      stepHi(e.shiftKey ? -1 : 1);
    } else if (k === "Enter") {
      e.preventDefault();
      activate();
    } else if (k === " ") {
      e.preventDefault();
      if (query) appendQuery(" ");
      else activate();
    } else if (k === "Backspace") {
      e.preventDefault();
      if (query) {
        query = query.slice(0, -1);
        if (!query) ringId++;
        hi = query && ring.length ? 0 : -1;
      } else if (stack.length > 0) popRing();
    } else if (k === "Escape") {
      e.preventDefault();
      if (query) clearQuery();
      else if (stack.length > 0) popRing();
      else onclose();
    } else if (k.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey && /[\p{L}\p{N}]/u.test(k)) {
      e.preventDefault();
      appendQuery(k);
    }
  }

  onMount(() => {
    const prev = document.activeElement;
    el?.focus();
    // onwheel via attributo è passivo in Svelte 5: serve un listener non passivo
    el?.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el?.removeEventListener("wheel", onWheel);
      if (timer) clearTimeout(timer);
      if (prev instanceof HTMLElement && prev.isConnected) prev.focus();
    };
  });
</script>

<svelte:window onresize={() => onclose()} onblur={() => onclose()} />

<div
  class="overlay"
  bind:this={el}
  role="menu"
  aria-label={title}
  aria-activedescendant={hi >= 0 ? "rm-petal-" + hi : undefined}
  tabindex="-1"
  onmousemove={onMove}
  onclick={onClick}
  oncontextmenu={onCtx}
  onkeydown={onKey}
  onmousedown={(e) => e.stopPropagation()}
>
  <span class="sr" aria-live="polite">{announce}</span>

  <div class="stage" style="left:{cx}px; top:{cy}px">
    <svg
      class="guide"
      width={R * 2 + 4}
      height={R * 2 + 4}
      style="left:{-R - 2}px; top:{-R - 2}px"
      aria-hidden="true"
    >
      <circle cx={R + 2} cy={R + 2} r={R} />
    </svg>

    <div class="chip" style="top:{-(R + 96)}px">
      <span class="chipTitle">{title}</span>
      {#if subtitle}<span class="chipSub">{subtitle}</span>{/if}
    </div>

    {#key ringId}
      <svg
        class="rays"
        width={(R + 120) * 2}
        height={(R + 120) * 2}
        style="left:{-(R + 120)}px; top:{-(R + 120)}px"
        aria-hidden="true"
      >
        {#each geo as g, i (g.p.item.id)}
          <line
            x1={g.rx1 + R + 120}
            y1={g.ry1 + R + 120}
            x2={g.rx2 + R + 120}
            y2={g.ry2 + R + 120}
            class:hi={hi === i}
            class:off={g.p.item.disabled}
          />
        {/each}
      </svg>
      <div class="ringbox">
        {#each geo as g, i (g.p.item.id)}
          <div
            class="wrap {g.hem}"
            class:implode={imploding}
            class:off={g.p.item.disabled}
            style="--tx:{g.tx}px; --ty:{g.ty}px; --i:{i}"
            data-petal={i}
            role={g.p.item.checked !== undefined ? "menuitemcheckbox" : "menuitem"}
            tabindex="-1"
            id="rm-petal-{i}"
            aria-disabled={g.p.item.disabled || undefined}
            aria-checked={g.p.item.checked !== undefined ? g.p.item.checked : undefined}
            aria-haspopup={g.p.item.children?.length ? "menu" : undefined}
            aria-label={g.p.label}
          >
            <div class="petal" class:hi={hi === i} class:danger={g.p.item.danger}>
              {#if g.p.item.icon}
                <svg class="ic" viewBox="0 0 24 24" aria-hidden="true">
                  <path d={g.p.item.icon} />
                </svg>
              {:else}
                <span class="pglyph">{g.p.item.label.trim().charAt(0).toUpperCase()}</span>
              {/if}
              {#if g.p.item.children?.length}
                <span class="dots" aria-hidden="true"><i></i><i></i><i></i></span>
              {/if}
              {#if g.p.item.badge}
                <span class="badge">{g.p.item.badge}</span>
              {/if}
              {#if g.p.item.checked}
                <span class="check" class:alt={!!g.p.item.badge} aria-hidden="true">
                  <svg viewBox="0 0 24 24"><path d="M20 6 9 17l-5-5" /></svg>
                </span>
              {/if}
            </div>
            <div
              class="label"
              class:hi={hi === i}
              class:danger={g.p.item.danger}
              style="--lx:{g.lx - g.tx}px; --ly:{g.ly - g.ty + g.dy}px"
            >
              {g.p.label}
            </div>
          </div>
        {/each}
      </div>
    {/key}

    <div
      class="hub"
      class:hasback={stack.length > 0}
      style="left:{-HUB}px; top:{-HUB}px; width:{HUB * 2}px; height:{HUB * 2}px"
    >
      {#if stack.length > 0}
        <div class="back">‹ Indietro</div>
      {/if}
      {#if thumb}
        <img class="cover" src={thumb} alt="" draggable="false" />
      {:else}
        <span class="hubGlyph">{hubLetter}</span>
      {/if}
    </div>

    <!-- top: sotto l'etichetta svasata della voce a ore 6 (che arriva a ~R+87) -->
    {#if hintText}
      <div class="hintbar" style="top:{R + 96}px">{hintText}</div>
    {/if}

    {#if query}
      <div class="qpill">{query}</div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 90;
    background: color-mix(in srgb, var(--bg) 45%, transparent);
    backdrop-filter: blur(2.5px) saturate(0.9);
    animation: fade 120ms ease-out both;
    user-select: none; outline: none; overflow: hidden;
  }
  @keyframes fade { from { opacity: 0; } }
  .sr {
    position: absolute; width: 1px; height: 1px; margin: -1px;
    overflow: hidden; clip-path: inset(50%); white-space: nowrap;
  }
  .stage { position: absolute; width: 0; height: 0; transition: left var(--ease), top var(--ease); }
  .guide { position: absolute; pointer-events: none; overflow: visible; }
  .guide circle {
    fill: none; stroke: var(--border); stroke-width: 1; stroke-dasharray: 2 8;
    opacity: 0.5; transition: r var(--ease);
  }

  /* chip del titolo, sopra l'anello */
  .chip {
    position: absolute; left: 0; transform: translate(-50%, -100%);
    display: flex; flex-direction: column; align-items: center; gap: 1px;
    padding: 6px 16px;
    background: color-mix(in srgb, var(--surface) 90%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border); border-radius: var(--r-lg); box-shadow: var(--shadow-sm);
    max-width: min(420px, calc(100vw - 28px));
    animation: fade 120ms ease-out both; transition: top var(--ease); pointer-events: none;
  }
  .chipTitle {
    font-family: var(--serif); font-size: 13px; color: var(--text);
    max-width: 34ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .chipSub {
    font-size: 11px; color: var(--faint);
    max-width: 42ch; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }

  /* petali */
  .ringbox { pointer-events: none; }
  .wrap {
    position: absolute; left: 0; top: 0; width: 0; height: 0;
    transform: translate(var(--tx), var(--ty));
    animation: bloom 170ms cubic-bezier(0.22, 0.9, 0.32, 1.18) both;
    animation-delay: calc(var(--i) * 14ms);
    outline: none;
  }
  .wrap.implode { animation: implode 110ms ease-in forwards; animation-delay: 0ms; }
  .wrap.off { opacity: 0.38; }
  @keyframes bloom {
    from { transform: translate(calc(var(--tx) * 0.35), calc(var(--ty) * 0.35)) scale(0.4); opacity: 0; }
  }
  @keyframes implode {
    to { transform: translate(calc(var(--tx) * 0.5), calc(var(--ty) * 0.5)) scale(0.5); opacity: 0; }
  }
  .petal {
    position: absolute; left: -27px; top: -27px; width: 54px; height: 54px; border-radius: 50%;
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    backdrop-filter: blur(10px);
    border: 1px solid var(--border); box-shadow: var(--shadow-md); color: var(--dim);
    display: flex; align-items: center; justify-content: center;
    pointer-events: auto; cursor: pointer; /* hit diretto (data-petal) sopra l'overlay */
    transition: transform var(--ease), border-color var(--ease), color var(--ease), box-shadow var(--ease);
  }
  .wrap.off .petal, .wrap.off .label { cursor: default; }
  .petal.hi {
    transform: scale(1.1); border-color: var(--accent); color: var(--accent);
    box-shadow: 0 0 0 4px var(--ring), var(--shadow-md);
  }
  .petal.danger { color: var(--danger); }
  .petal.danger.hi {
    border-color: var(--danger);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--danger) 22%, transparent), var(--shadow-md);
  }
  .ic { width: 22px; height: 22px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linecap: round; stroke-linejoin: round; }
  .pglyph { font-family: var(--serif); font-size: 16px; font-weight: 600; }
  .dots { position: absolute; bottom: 7px; left: 50%; transform: translateX(-50%); display: flex; gap: 3px; opacity: 0.6; }
  .dots i { display: block; width: 2px; height: 2px; border-radius: 50%; background: currentColor; }
  .dots i:nth-child(2) { transform: translateY(1px); }
  .badge {
    position: absolute; top: -5px; right: -7px;
    font-size: 9.5px; font-weight: 700; line-height: 1; padding: 2px 5px;
    border-radius: var(--r-pill); background: var(--accent-soft); color: var(--accent);
    border: 1px solid var(--accent-soft2);
  }
  .check {
    position: absolute; top: 1px; right: 1px; width: 7px; height: 7px; border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--surface) 88%, transparent);
    display: flex; align-items: center; justify-content: center;
  }
  .check.alt { right: auto; left: 1px; }
  .check svg { width: 5px; height: 5px; fill: none; stroke: var(--on-accent); stroke-width: 4; stroke-linecap: round; stroke-linejoin: round; }

  /* etichette a callout radiale: ancora (--lx/--ly) calcolata nello script,
     allineamento per emisfero (r cresce a destra, l a sinistra, c centrata) */
  .label {
    position: absolute; left: var(--lx, 0px); top: var(--ly, 0px);
    font-size: 13px; line-height: 1.25; color: var(--text);
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(6px);
    border: 1px solid var(--border-soft);
    box-shadow: var(--shadow-sm);
    padding: 3px 10px; border-radius: var(--r-pill);
    white-space: nowrap; max-width: 170px; /* combacia con LABEL_W nello script */
    overflow: hidden; text-overflow: ellipsis;
    pointer-events: auto; cursor: pointer;
    transition: color var(--ease);
  }
  .wrap.r .label { transform: translateY(-50%); }
  .wrap.l .label { transform: translate(-100%, -50%); }
  .wrap.c .label { transform: translate(-50%, -50%); }
  .label.hi { color: var(--accent); font-weight: 700; border-color: var(--accent-soft2); }
  .label.danger { color: var(--danger); }

  /* raggi petalo→etichetta (sotto i petali, sopra la guida tratteggiata) */
  .rays { position: absolute; pointer-events: none; overflow: visible; animation: fade 170ms ease-out both; }
  .rays line { stroke: var(--faint); stroke-width: 1; opacity: 0.45; transition: stroke var(--ease), opacity var(--ease); }
  .rays line.hi { stroke: var(--accent); stroke-width: 1.5; opacity: 0.95; }
  .rays line.off { opacity: 0.15; }

  /* mozzo */
  .hub {
    /* left/top/width/height inline, derivati dalla costante HUB */
    position: absolute; border-radius: 50%;
    background: color-mix(in srgb, var(--surface) 88%, transparent);
    backdrop-filter: blur(10px);
    border: 1px solid color-mix(in srgb, var(--border) 55%, var(--dim));
    box-shadow: var(--shadow-md);
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    gap: 3px; text-align: center; overflow: hidden;
    animation: hubIn 170ms cubic-bezier(0.22, 0.9, 0.32, 1.18) both;
    pointer-events: none;
  }
  @keyframes hubIn { from { transform: scale(0.6); opacity: 0; } }
  .back { position: absolute; top: 9px; left: 0; width: 100%; font-size: 11px; font-weight: 600; color: var(--accent); }
  .cover { width: 54px; height: 54px; border-radius: 12px; object-fit: cover; border: 1px solid var(--border-soft); }
  .hub.hasback .cover { width: 44px; height: 44px; }
  .hubGlyph { font-family: var(--serif); font-size: 26px; font-weight: 600; line-height: 1; color: var(--accent); }

  /* descrizione della voce evidenziata: barra leggibile sotto l'anello */
  .hintbar {
    position: absolute; left: 0; transform: translateX(-50%);
    font-size: 13px; line-height: 1.45; color: var(--dim); text-align: center;
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    backdrop-filter: blur(8px);
    border: 1px solid var(--border-soft); border-radius: var(--r-md);
    box-shadow: var(--shadow-md);
    padding: 8px 18px;
    /* si adatta al testo della voce corrente: cresce fino al massimo, poi va a capo — mai troncato */
    width: max-content; max-width: min(560px, calc(100vw - 28px));
    pointer-events: none; animation: fade 120ms ease-out both;
  }

  /* eco della ricerca (type-to-filter) */
  .qpill {
    position: absolute; top: 56px; left: 0; transform: translateX(-50%);
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 12px; color: var(--text);
    background: var(--field); border: 1px solid var(--border); border-radius: var(--r-pill);
    padding: 3px 10px; box-shadow: var(--shadow-sm);
    white-space: nowrap; max-width: 240px; overflow: hidden; text-overflow: ellipsis;
    pointer-events: none;
  }
  .qpill::after {
    content: ""; display: inline-block; width: 1px; height: 11px; margin-left: 2px;
    vertical-align: -1px; background: var(--accent);
    animation: blink 1s steps(1) infinite;
  }
  @keyframes blink { 50% { opacity: 0; } }

  @media (prefers-reduced-motion: reduce) {
    .overlay, .wrap, .hub, .chip, .hintbar, .rays { animation: none !important; }
    .qpill::after { animation: none; }
    .stage, .chip, .petal, .label, .guide circle { transition: none; }
    .petal.hi { transform: none; }
  }
</style>
