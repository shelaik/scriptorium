<script lang="ts">
  import { shareTo, revealDocument, type ShareTarget } from "$lib/share";

  let {
    ids,
    label = "Documento PDF",
    link = null,
    compact = false,
    variant = "button",
    onstatus,
    onclose,
  }: {
    ids: number[];
    label?: string;
    /** Original paper link (e.g. DOI) appended to the shared message. */
    link?: string | null;
    compact?: boolean;
    variant?: "button" | "menuitem";
    onstatus?: (s: string) => void;
    onclose?: () => void;
  } = $props();

  let open = $state(false);
  let pos = $state<{ x: number; y: number } | null>(null);
  let busy = $state(false);

  const TARGETS: { key: ShareTarget; name: string }[] = [
    { key: "whatsapp", name: "WhatsApp" },
    { key: "teams", name: "Microsoft Teams" },
    { key: "gmail", name: "Gmail" },
    { key: "outlook", name: "Outlook" },
  ];

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (open) {
      open = false;
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const width = 216;
    let x = r.left;
    if (x + width + 8 > window.innerWidth) x = window.innerWidth - width - 8;
    let y = r.bottom + 6;
    // If there isn't room below, open upward.
    if (y + 250 > window.innerHeight) y = Math.max(8, r.top - 250);
    pos = { x: Math.max(8, x), y };
    open = true;
  }

  async function pick(target: ShareTarget) {
    if (busy || !ids.length) return;
    busy = true;
    open = false;
    onstatus?.("Preparazione condivisione…");
    try {
      const r = await shareTo(target, ids, label, link);
      onstatus?.(r.note);
    } catch (e) {
      onstatus?.("Errore condivisione: " + e);
    } finally {
      busy = false;
      onclose?.();
    }
  }

  async function openFolder() {
    open = false;
    if (ids.length) {
      try {
        await revealDocument(ids[0]);
      } catch {
        onstatus?.("Questo elemento non ha un file da mostrare");
      }
    }
    onclose?.();
  }
</script>

<svelte:window onclick={() => (open = false)} />

<button
  class="sharebtn"
  class:compact
  class:menuitem={variant === "menuitem"}
  onclick={toggle}
  disabled={busy || !ids.length}
  title="Condividi via WhatsApp, Teams, Gmail o Outlook"
>
  {busy ? "…" : "Condividi"}
</button>

{#if open && pos}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="sharemenu" style="left:{pos.x}px; top:{pos.y}px" onclick={(e) => e.stopPropagation()}>
    {#each TARGETS as t (t.key)}
      <button class="shareitem" onclick={() => pick(t.key)}>{t.name}</button>
    {/each}
    <div class="sharesep"></div>
    <button class="shareitem" onclick={openFolder}>Apri cartella del file</button>
  </div>
{/if}

<style>
  .sharebtn {
    background: var(--field);
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 7px;
    padding: 5px 11px;
    font-size: 14px;
    cursor: pointer;
    white-space: nowrap;
  }
  .sharebtn.compact {
    background: var(--surface);
    font-size: 12px;
  }
  .sharebtn:hover:not(:disabled) {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .sharebtn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  /* "menuitem" variant: blends into the card actions menu like a .medit row */
  .sharebtn.menuitem {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    color: var(--text);
    border: none;
    border-bottom: 1px solid var(--border-soft);
    border-radius: 6px;
    padding: 6px 4px;
    font-size: 13px;
    font-weight: 400;
    margin-bottom: 6px;
  }
  .sharebtn.menuitem:hover:not(:disabled) {
    background: var(--accent-soft);
    border-color: transparent;
    border-bottom-color: var(--border-soft);
  }
  .sharemenu {
    position: fixed;
    z-index: 100;
    width: 216px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 6px;
    box-shadow: 0 10px 30px rgba(44, 46, 53, 0.2);
  }
  .shareitem {
    display: block;
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
  .shareitem:hover {
    background: var(--accent-soft);
  }
  .sharesep {
    height: 1px;
    background: var(--border-soft);
    margin: 5px 4px;
  }
</style>
