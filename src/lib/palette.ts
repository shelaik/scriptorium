// Shared types for the command palette (CommandPalette.svelte).

export interface PaletteEntry {
  id: string;
  title: string; // Italian
  hint?: string; // right-aligned dim explanation
  section: string; // group header, e.g. "Azioni", "Documenti", "Vai a", "Aspetto"
  keywords?: string; // extra fuzzy-match corpus (authors, synonyms)
  shortcut?: string; // e.g. "Ctrl+K" shown as a key chip
  run: () => void;
}
