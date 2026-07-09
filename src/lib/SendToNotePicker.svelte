<script lang="ts">
  import { listNotes, createNote, appendToNote, deleteNote, type NoteMeta } from "$lib/api";
  import { buildQuoteBlock, type NotePayload } from "$lib/notecite";

  let {
    payload,
    pos,
    currentNote = null,
    onstatus,
    onclose,
    ondone,
  }: {
    /** What to append and how to attribute it. */
    payload: NotePayload;
    /** Anchor (top-left) in viewport coordinates. */
    pos: { x: number; y: number };
    /** The note open in the Notes view right now, if any. */
    currentNote?: { slug: string; title: string } | null;
    onstatus?: (s: string) => void;
    onclose: () => void;
    ondone?: (info: { slug: string; title: string }) => void;
  } = $props();

  let notes = $state<NoteMeta[]>([]);
  let busy = $state(false);
  let loaded = $state(false);
  // Ignore the very click that opened us, so the popover doesn't self-close.
  let armed = $state(false);
  $effect(() => {
    const t = setTimeout(() => (armed = true), 60);
    return () => clearTimeout(t);
  });
  function guardedClose(e: MouseEvent) {
    if (!armed) return;
    // A click inside the popover (padding, etc.) keeps it open; buttons close it
    // themselves after acting.
    if ((e.target as HTMLElement | null)?.closest(".stnp")) return;
    onclose();
  }
  // Swallow Escape in the CAPTURE phase so it closes only this popover — otherwise
  // the PDF viewer's own window keydown (registered earlier, fires first) would
  // treat Escape as "close the reader".
  function onKeyCapture(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onclose();
    }
  }

  // Recent notes for the menu, minus the "open" one (shown separately on top).
  const recent = $derived(
    notes.filter((n) => n.slug !== currentNote?.slug).slice(0, 6),
  );

  $effect(() => {
    listNotes()
      .then((n) => (notes = n))
      .catch(() => (notes = []))
      .finally(() => (loaded = true));
  });

  // Clamp the popover into the viewport.
  const box = $derived.by(() => {
    const w = 260;
    const h = 320;
    let x = Math.max(8, pos.x);
    if (x + w + 8 > window.innerWidth) x = Math.max(8, window.innerWidth - w - 8);
    let y = pos.y;
    if (y + h + 8 > window.innerHeight) y = Math.max(8, window.innerHeight - h - 8);
    return { x, y, w };
  });

  async function append(slug: string, title: string) {
    if (busy) return;
    busy = true;
    try {
      await appendToNote(slug, buildQuoteBlock(payload));
      onstatus?.(`Aggiunto a «${title}» ✓`);
      ondone?.({ slug, title });
      onclose();
    } catch (e) {
      onstatus?.("Errore: non aggiunto all'appunto (" + e + ")");
      busy = false;
    }
  }

  async function toNew() {
    if (busy) return;
    busy = true;
    const name = (payload.title || "Appunti").trim() || "Appunti";
    let slug: string | null = null;
    try {
      slug = await createNote(name);
      await appendToNote(slug, buildQuoteBlock(payload));
      onstatus?.(`Creato l'appunto «${name}» ✓`);
      ondone?.({ slug, title: name });
      onclose();
    } catch (e) {
      // Roll back the just-created (content-less) note so nothing dangles.
      if (slug) {
        try {
          await deleteNote(slug);
        } catch {
          /* best-effort */
        }
      }
      onstatus?.("Errore: appunto non creato (" + e + ")");
      busy = false;
    }
  }
</script>

<svelte:window onclick={guardedClose} onkeydowncapture={onKeyCapture} />

<div class="stnp" style="left:{box.x}px; top:{box.y}px; width:{box.w}px">
  <div class="stnp-head">Manda agli Appunti</div>

  {#if currentNote}
    <button class="stnp-item open" disabled={busy} onclick={() => append(currentNote.slug, currentNote.title)}>
      <span class="stnp-dot">▸</span>
      <span class="stnp-t">{currentNote.title}</span>
      <span class="stnp-tag">aperta</span>
    </button>
  {/if}

  {#if recent.length}
    <div class="stnp-lbl">Recenti</div>
    {#each recent as n (n.slug)}
      <button class="stnp-item" disabled={busy} onclick={() => append(n.slug, n.title)}>
        <span class="stnp-t">{n.title}</span>
      </button>
    {/each}
  {:else if loaded && !currentNote}
    <div class="stnp-empty">Nessun appunto ancora — creane uno qui sotto.</div>
  {/if}

  <div class="stnp-sep"></div>
  <button class="stnp-item new" disabled={busy} onclick={toNew}>
    ＋ Nuovo appunto{payload.title ? ` — «${payload.title}»` : ""}
  </button>
</div>

<style>
  .stnp {
    position: fixed;
    z-index: 200;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 6px;
    box-shadow: 0 12px 34px rgba(44, 46, 53, 0.24);
    max-height: 340px;
    overflow-y: auto;
  }
  .stnp-head {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--dim);
    padding: 4px 8px 6px;
  }
  .stnp-lbl {
    font-size: 11px;
    color: var(--dim);
    padding: 6px 8px 2px;
  }
  .stnp-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    color: var(--text);
    padding: 7px 8px;
    font-size: 13px;
    cursor: pointer;
    border-radius: 6px;
  }
  .stnp-item:hover:not(:disabled) {
    background: var(--accent-soft);
  }
  .stnp-item:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .stnp-item.new {
    color: var(--accent);
  }
  .stnp-t {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .stnp-dot {
    color: var(--accent);
  }
  .stnp-tag {
    font-size: 10.5px;
    color: var(--dim);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 1px 6px;
  }
  .stnp-empty {
    font-size: 12px;
    color: var(--dim);
    padding: 6px 8px;
  }
  .stnp-sep {
    height: 1px;
    background: var(--border-soft);
    margin: 5px 4px;
  }
</style>
