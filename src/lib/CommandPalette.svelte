<script lang="ts">
  import type { PaletteEntry } from "$lib/palette";

  let {
    entries,
    onclose,
    placeholder = "Cerca comandi, documenti, filtri…",
  }: { entries: PaletteEntry[]; onclose: () => void; placeholder?: string } = $props();

  const RECENTS_KEY = "scriptorium-recents";
  const CAP = 40;

  interface Seg {
    text: string;
    hit: boolean;
  }
  interface Row {
    entry: PaletteEntry;
    segs: Seg[];
    index: number;
  }
  interface Group {
    section: string;
    rows: Row[];
  }

  let query = $state("");
  let selected = $state(0);
  let inputEl = $state<HTMLInputElement | null>(null);

  function loadRecents(): string[] {
    try {
      const raw = localStorage.getItem(RECENTS_KEY);
      const arr: unknown = raw ? JSON.parse(raw) : [];
      return Array.isArray(arr) ? arr.filter((x): x is string => typeof x === "string") : [];
    } catch {
      return [];
    }
  }
  let recents = $state<string[]>(loadRecents());

  /** Lowercase + strip diacritics (NFD), so "perché" matches "perche". */
  function fold(s: string): string {
    return s
      .normalize("NFD")
      .replace(/[\u0300-\u036f]/g, "")
      .toLowerCase();
  }

  function isCombining(code: number): boolean {
    return code >= 0x0300 && code <= 0x036f;
  }

  /**
   * Fold cluster by cluster (code point + trailing combining marks), keeping,
   * for each folded char, the [start, end) span of its original grapheme so a
   * highlight always covers the whole visible glyph (NFC and NFD alike).
   */
  function foldMap(s: string): { folded: string; spans: [number, number][] } {
    let folded = "";
    const spans: [number, number][] = [];
    let i = 0;
    while (i < s.length) {
      const cp = s.codePointAt(i) ?? 0;
      let end = i + (cp > 0xffff ? 2 : 1);
      while (end < s.length && isCombining(s.charCodeAt(end))) end++;
      const f = fold(s.slice(i, end)); // may yield 0 chars (lone combining mark)
      for (let k = 0; k < f.length; k++) spans.push([i, end]);
      folded += f;
      i = end;
    }
    return { folded, spans };
  }

  /** Greedy subsequence match: +3 word-start hit, +2 consecutive, -0.5 per gap char. */
  function fuzzy(q: string, text: string): { score: number; indices: number[] } | null {
    let score = 0;
    const indices: number[] = [];
    let from = 0;
    let prev = -2;
    for (const ch of q) {
      const at = text.indexOf(ch, from);
      if (at === -1) return null;
      if (indices.length > 0 && at > prev + 1) score -= 0.5 * (at - prev - 1);
      if (at === prev + 1) score += 2;
      if (at === 0 || !/[a-z0-9]/.test(text[at - 1])) score += 3;
      indices.push(at);
      prev = at;
      from = at + 1;
    }
    return { score, indices };
  }

  /** Split a title into contiguous hit / non-hit segments (rendered without {@html}). */
  function segsOf(title: string, hits: Set<number>): Seg[] {
    const segs: Seg[] = [];
    for (let i = 0; i < title.length; i++) {
      const hit = hits.has(i);
      const last = segs[segs.length - 1];
      if (last && last.hit === hit) last.text += title[i];
      else segs.push({ text: title[i], hit });
    }
    return segs;
  }

  function plain(e: PaletteEntry): Seg[] {
    return [{ text: e.title, hit: false }];
  }

  function chips(shortcut: string): string[] {
    return shortcut.split("+").map((c) => c.trim()).filter(Boolean);
  }

  const view = $derived.by<{ groups: Group[]; flat: Row[] }>(() => {
    const q = fold(query).replace(/\s+/g, "");
    const groups: Group[] = [];
    const flat: Row[] = [];

    const push = (section: string, entry: PaletteEntry, segs: Seg[]) => {
      if (flat.length >= CAP) return;
      let g = groups[groups.length - 1];
      if (!g || g.section !== section) {
        g = { section, rows: [] };
        groups.push(g);
      }
      const row: Row = { entry, segs, index: flat.length };
      g.rows.push(row);
      flat.push(row);
    };

    if (!q) {
      // Recents first (stale ids silently dropped), then up to 5 per section in given order.
      const byId = new Map(entries.map((e) => [e.id, e]));
      const seen = new Set<string>();
      for (const id of recents) {
        if (seen.size >= 6) break;
        const e = byId.get(id);
        if (!e || seen.has(id)) continue;
        seen.add(id);
        push("Recenti", e, plain(e));
      }
      const order: string[] = [];
      const bySec = new Map<string, PaletteEntry[]>();
      for (const e of entries) {
        let arr = bySec.get(e.section);
        if (!arr) {
          arr = [];
          bySec.set(e.section, arr);
          order.push(e.section);
        }
        if (arr.length < 5) arr.push(e);
      }
      for (const s of order) for (const e of bySec.get(s)!) push(s, e, plain(e));
    } else {
      type Scored = { entry: PaletteEntry; segs: Seg[]; score: number };
      const order: string[] = [];
      const bySec = new Map<string, Scored[]>();
      for (const e of entries) {
        const tm = foldMap(e.title);
        const corpus = e.keywords ? tm.folded + " " + fold(e.keywords) : tm.folded;
        const m = fuzzy(q, corpus);
        if (!m) continue;
        const hits = new Set<number>();
        for (const i of m.indices) {
          if (i >= tm.folded.length) continue; // keyword hit, not shown in title
          const [a, b] = tm.spans[i];
          for (let k = a; k < b; k++) hits.add(k);
        }
        let arr = bySec.get(e.section);
        if (!arr) {
          arr = [];
          bySec.set(e.section, arr);
          order.push(e.section);
        }
        arr.push({ entry: e, segs: segsOf(e.title, hits), score: m.score });
      }
      const sections = order
        .map((s) => {
          const items = bySec.get(s)!;
          items.sort((a, b) => b.score - a.score);
          return { section: s, items, best: items[0].score };
        })
        .sort((a, b) => b.best - a.best);

      // Cap total rows, allocating proportionally across sections (grouping preserved).
      const total = sections.reduce((n, s) => n + s.items.length, 0);
      let quotas = sections.map((s) => s.items.length);
      if (total > CAP) {
        quotas = sections.map((s) => Math.max(1, Math.floor((s.items.length / total) * CAP)));
        let sum = quotas.reduce((a, b) => a + b, 0);
        let guard = 0;
        while (sum < CAP && guard++ < CAP) {
          for (let i = 0; i < sections.length && sum < CAP; i++) {
            if (quotas[i] < sections[i].items.length) {
              quotas[i]++;
              sum++;
            }
          }
        }
        for (let i = sections.length - 1; sum > CAP && i >= 0; i--) {
          while (quotas[i] > 1 && sum > CAP) {
            quotas[i]--;
            sum--;
          }
        }
      }
      sections.forEach((s, i) => {
        for (const it of s.items.slice(0, quotas[i])) push(s.section, it.entry, it.segs);
      });
    }
    return { groups, flat };
  });

  function runEntry(entry: PaletteEntry) {
    const next = [entry.id, ...recents.filter((id) => id !== entry.id)].slice(0, 12);
    recents = next;
    try {
      localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
    } catch {
      // localStorage unavailable: recents just won't persist
    }
    entry.run();
    onclose();
  }

  function onKey(e: KeyboardEvent) {
    if (e.isComposing) return;
    const len = view.flat.length;
    switch (e.key) {
      case "Escape":
        e.preventDefault();
        e.stopPropagation();
        if (query !== "") {
          query = "";
          selected = 0;
        } else {
          onclose();
        }
        break;
      case "ArrowDown":
        e.preventDefault();
        e.stopPropagation();
        if (len > 0) selected = (selected + 1) % len;
        break;
      case "ArrowUp":
        e.preventDefault();
        e.stopPropagation();
        if (len > 0) selected = (selected - 1 + len) % len;
        break;
      case "PageDown":
        e.preventDefault();
        e.stopPropagation();
        if (len > 0) selected = Math.min(len - 1, selected + 8);
        break;
      case "PageUp":
        e.preventDefault();
        e.stopPropagation();
        if (len > 0) selected = Math.max(0, selected - 8);
        break;
      case "Enter": {
        e.preventDefault();
        e.stopPropagation();
        const row = view.flat[selected];
        if (row) runEntry(row.entry);
        break;
      }
      case "Tab":
        e.preventDefault();
        break;
    }
  }

  function stop(e: Event) {
    e.stopPropagation();
  }

  $effect(() => {
    inputEl?.focus();
  });

  $effect(() => {
    if (selected >= view.flat.length && selected !== 0) selected = 0;
  });

  $effect(() => {
    void view.flat.length;
    const el = document.getElementById("pal-opt-" + selected);
    el?.scrollIntoView({ block: "nearest" });
  });
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="overlay"
  role="presentation"
  onmousedown={(e) => {
    e.stopPropagation();
    if (e.target === e.currentTarget) onclose();
  }}
  onclick={stop}
  onpointerdown={stop}
  onmouseup={stop}
>
  <div class="card" role="dialog" aria-modal="true" aria-label="Palette comandi">
    <div class="head">
      <span class="icon" aria-hidden="true">
        <svg
          width="16"
          height="16"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <circle cx="11" cy="11" r="7"></circle>
          <line x1="21" y1="21" x2="16.2" y2="16.2"></line>
        </svg>
      </span>
      <input
        bind:this={inputEl}
        bind:value={query}
        oninput={() => (selected = 0)}
        type="text"
        {placeholder}
        aria-label="Cerca comandi"
        aria-controls="pal-list"
        aria-activedescendant={view.flat.length > 0 ? "pal-opt-" + selected : undefined}
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
      />
    </div>

    <div class="list" id="pal-list" role="listbox" aria-label="Risultati">
      {#each view.groups as group (group.section)}
        <div class="sec" role="presentation">{group.section}</div>
        {#each group.rows as row (row.index)}
          <!-- keyboard interaction lives on the window handler (aria-activedescendant pattern) -->
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <div
            class="row"
            class:sel={row.index === selected}
            role="option"
            id={"pal-opt-" + row.index}
            aria-selected={row.index === selected}
            tabindex="-1"
            onmousemove={() => {
              if (selected !== row.index) selected = row.index;
            }}
            onclick={() => runEntry(row.entry)}
          >
            <span class="title"
              >{#each row.segs as seg, i (i)}{#if seg.hit}<span class="hit">{seg.text}</span
                >{:else}{seg.text}{/if}{/each}</span
            >
            {#if row.entry.hint || row.entry.shortcut}
              <span class="right">
                {#if row.entry.hint}<span class="hint">{row.entry.hint}</span>{/if}
                {#if row.entry.shortcut}
                  <span class="keys">
                    {#each chips(row.entry.shortcut) as c, i (i)}<kbd>{c}</kbd>{/each}
                  </span>
                {/if}
              </span>
            {/if}
          </div>
        {/each}
      {/each}
      {#if view.flat.length === 0}
        <div class="empty">Nessun risultato</div>
      {/if}
    </div>

    <div class="foot">↑↓ naviga · Invio esegue · Esc chiude</div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 95;
    display: flex; justify-content: center; align-items: flex-start;
    background: color-mix(in srgb, var(--bg) 45%, transparent);
    backdrop-filter: blur(2px);
  }
  .card {
    display: flex; flex-direction: column;
    width: min(620px, 92vw); margin-top: 14vh; max-height: calc(86vh - 24px);
    background: color-mix(in srgb, var(--surface) 92%, transparent);
    backdrop-filter: blur(14px);
    border: 1px solid var(--border); border-radius: var(--r-lg);
    box-shadow: var(--shadow-lg); overflow: hidden;
    animation: pal-in 120ms cubic-bezier(0.4, 0, 0.2, 1);
  }
  @keyframes pal-in {
    from { opacity: 0; transform: scale(0.97); }
    to { opacity: 1; transform: scale(1); }
  }
  @media (prefers-reduced-motion: reduce) {
    .card { animation: none; }
  }
  .head {
    display: flex; align-items: center; gap: 10px;
    padding: 13px 16px; border-bottom: 1px solid var(--border-soft);
  }
  .icon { display: flex; flex: none; color: var(--faint); }
  input {
    flex: 1; min-width: 0; padding: 0;
    background: transparent; border: none; outline: none;
    font-family: var(--sans); font-size: 15px; color: var(--text);
  }
  input::placeholder { color: var(--faint); }
  input:focus-visible { box-shadow: none; }
  .list { max-height: 46vh; overflow-y: auto; overscroll-behavior: contain; padding: 6px 0 8px; }
  .sec {
    padding: 10px 16px 4px; font-size: 10.5px; font-weight: 700;
    letter-spacing: 0.8px; text-transform: uppercase; color: var(--faint);
  }
  .row {
    display: flex; align-items: center; gap: 10px;
    padding: 7px 16px 7px 14px; border-left: 2px solid transparent;
    cursor: pointer; user-select: none;
  }
  .row.sel { background: var(--accent-soft); border-left-color: var(--accent); }
  .title {
    flex: 0 1 auto; min-width: 0; font-size: 13.5px; color: var(--text);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .row.sel .title { color: var(--accent); }
  .hit {
    color: var(--accent);
    text-decoration: underline; text-decoration-thickness: 1px; text-underline-offset: 2px;
  }
  .right {
    margin-left: auto; flex: 0 3 auto; min-width: 0;
    display: flex; align-items: center; gap: 8px;
  }
  .hint {
    flex: 0 1 auto; min-width: 0; font-size: 12px; color: var(--faint);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; text-align: right;
  }
  .keys { flex: none; display: flex; gap: 3px; }
  kbd {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 10.5px; line-height: 1.5; color: var(--dim);
    background: var(--field); border: 1px solid var(--border);
    border-radius: 5px; padding: 0 5px;
  }
  .empty { padding: 20px 16px; font-size: 13px; color: var(--dim); }
  .foot {
    padding: 8px 16px; font-size: 10.5px; color: var(--faint);
    border-top: 1px solid var(--border-soft);
  }
</style>
