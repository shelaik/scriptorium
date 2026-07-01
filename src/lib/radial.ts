/** Voce del menu radiale (RadialMenu.svelte). */
export interface RadialItem {
  id: string;
  label: string;          // italiano, breve
  hint?: string;          // descrizione mostrata nel mozzo quando evidenziata
  icon?: string;          // attributo "d" di un path SVG 24x24, stroke-based
  danger?: boolean;
  disabled?: boolean;
  checked?: boolean;      // pallino ✓ sul petalo
  badge?: string;         // mini badge testuale (es. un conteggio)
  children?: RadialItem[];// ramo → apre un sotto-anello
  action?: () => void;    // foglia → eseguita, poi il menu si chiude
}
