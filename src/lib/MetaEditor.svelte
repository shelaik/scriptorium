<script lang="ts">
  import { onMount } from "svelte";
  import { getDocumentMeta, updateDocumentMetadata, type EditableMeta } from "$lib/api";

  let {
    id,
    onClose,
    onSaved,
  }: { id: number; onClose: () => void; onSaved: () => void } = $props();

  let m = $state<EditableMeta>({
    title: "",
    authors: [],
    year: null,
    venue: "",
    doi: "",
    abstract_text: "",
    notes: "",
    summary: null,
  });
  let authorsText = $state("");
  let loading = $state(true);
  let saving = $state(false);
  let error = $state("");

  onMount(async () => {
    try {
      const d = await getDocumentMeta(id);
      m = { ...d };
      authorsText = (d.authors ?? []).join("\n");
    } catch (e) {
      error = "Errore caricamento: " + e;
    } finally {
      loading = false;
    }
  });

  async function save() {
    saving = true;
    error = "";
    try {
      const authors = authorsText
        .split("\n")
        .map((s) => s.trim())
        .filter(Boolean);
      await updateDocumentMetadata(id, { ...m, authors });
      onSaved();
    } catch (e) {
      error = "" + e;
      saving = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="back" onmousedown={(e) => { if (e.target === e.currentTarget) onClose(); }} role="presentation">
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="modal" onclick={(e) => e.stopPropagation()}>
    <h2>Modifica metadati</h2>
    {#if loading}
      <p class="dim">Caricamento…</p>
    {:else}
      <label>Titolo<input bind:value={m.title} /></label>
      <label>
        Autori (uno per riga)
        <textarea rows="4" bind:value={authorsText} placeholder="Nome Cognome"></textarea>
      </label>
      <div class="row2">
        <label>Anno<input type="number" bind:value={m.year} /></label>
        <label>DOI<input bind:value={m.doi} /></label>
      </div>
      <label>Rivista / Venue<input bind:value={m.venue} /></label>
      <label>Abstract<textarea rows="5" bind:value={m.abstract_text}></textarea></label>
      <label>Note<textarea rows="4" bind:value={m.notes} placeholder="Appunti personali su questo documento"></textarea></label>
      {#if m.summary}
        <label>Riassunto (AI)<textarea rows="5" readonly value={m.summary}></textarea></label>
      {/if}
      {#if error}<p class="err">{error}</p>{/if}
      <div class="actions">
        <button class="ghost" onclick={onClose}>Annulla</button>
        <button class="primary" onclick={save} disabled={saving}>{saving ? "…" : "Salva"}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .back {
    position: fixed; inset: 0; z-index: 80;
    background: rgba(44, 46, 53, 0.4);
    display: flex; align-items: center; justify-content: center; padding: 24px;
  }
  .modal {
    width: 560px; max-width: 100%; max-height: 92vh; overflow: auto;
    background: var(--surface); border: 1px solid var(--border); border-radius: var(--r-lg, 14px); padding: 22px 24px;
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(44, 46, 53, 0.22));
    resize: both; min-width: 380px; min-height: 260px;
  }
  h2 { margin: 0 0 16px; font-size: 18px; font-family: var(--serif); font-weight: 600; color: var(--text); }
  .dim { color: var(--dim); }
  label {
    display: flex; flex-direction: column; gap: 5px;
    font-size: 12px; color: var(--dim); margin-bottom: 13px;
  }
  input, textarea {
    background: var(--field); border: 1px solid var(--border); color: var(--text);
    border-radius: 7px; padding: 8px 10px; font-size: 14px; font-family: inherit;
    outline: none; resize: vertical;
  }
  input:focus, textarea:focus { border-color: var(--accent); }
  .row2 { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
  .err { color: var(--danger); font-size: 13px; margin: 4px 0 0; }
  .actions { display: flex; justify-content: flex-end; gap: 10px; margin-top: 8px; }
  button {
    border-radius: 8px; padding: 9px 18px; font-size: 14px; font-weight: 600; cursor: pointer;
    border: 1px solid var(--border); background: transparent; color: var(--accent);
  }
  button.primary { background: var(--accent); color: var(--on-accent); border: none; }
  button.primary:disabled { opacity: 0.6; cursor: default; }
</style>
