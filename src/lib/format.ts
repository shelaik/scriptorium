// Date, time and number formatting for the whole app, in ONE place.
//
// Prima ogni punto chiamava `toLocaleDateString("it-IT", …)` col codice lingua
// scritto a mano: cinque copie sparse fra la pagina principale, i progetti
// LaTeX e la Plancia. Ora il codice lingua lo decide `$lib/i18n` e qui non
// compare nessun letterale fuori dalla tabella qui sotto.
//
// I formattatori LEGGONO la lingua a ogni chiamata (e la memorizzano per
// lingua): chiamati dentro un template Svelte, si aggiornano da soli quando
// l'utente cambia lingua, senza ricaricare nulla.

import { i18n, type Locale } from "$lib/i18n/index.svelte";

/** Il BCP-47 usato dai formattatori per ciascuna lingua dell'interfaccia.
 *  `en-GB` e non `en-US`: mantiene giorno-mese-anno e l'orologio a 24 ore, cioe'
 *  le convenzioni che l'utente vede gia' oggi, invece di ribaltarle in
 *  «Jul 28» e «2:30 PM» solo perche' l'etichetta di un pulsante e' cambiata. */
const BCP47: Record<Locale, string> = { it: "it-IT", en: "en-GB" };

type Kind = "shortDate" | "dateTime" | "dayMonth" | "clock";

const OPTS: Record<Kind, Intl.DateTimeFormatOptions> = {
  /** «10 lug 2026» — data compatta, per elenchi fitti. */
  shortDate: { day: "numeric", month: "short", year: "numeric" },
  /** «10 lug 2026, 14:32» — data e ora, per le righe di dettaglio. */
  dateTime: { dateStyle: "medium", timeStyle: "short" },
  /** «10 lug» — giorno e mese, quando l'anno e' implicito nel contesto. */
  dayMonth: { day: "numeric", month: "short" },
  /** Orologio a 24 ore. `hour12` e' esplicito di proposito: il valore
   *  predefinito dipende dalla lingua, e il registro della Plancia dev'essere
   *  leggibile allo stesso modo in entrambe. */
  clock: { hour: "2-digit", minute: "2-digit", second: "2-digit", hour12: false },
};

const cache = new Map<string, Intl.DateTimeFormat>();

function fmt(kind: Kind): Intl.DateTimeFormat {
  const loc = BCP47[i18n.locale] ?? BCP47.it;
  const id = `${kind}:${loc}`;
  let f = cache.get(id);
  if (!f) {
    f = new Intl.DateTimeFormat(loc, OPTS[kind]);
    cache.set(id, f);
  }
  return f;
}

/** Un formattatore non deve MAI far saltare una vista: timestamp assurdi o
 *  `Invalid Date` tornano come trattino. */
function safe(kind: Kind, ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return "—";
  try {
    return fmt(kind).format(new Date(ms));
  } catch {
    return "—";
  }
}

export const fmtDateShort = (ms: number | null | undefined) => safe("shortDate", ms);
export const fmtDateTime = (ms: number | null | undefined) => safe("dateTime", ms);
export const fmtDayMonth = (ms: number | null | undefined) => safe("dayMonth", ms);
export const fmtClock = (ms: number | null | undefined) => safe("clock", ms);

/** Numeri col separatore delle migliaia della lingua corrente. */
export const fmtNum = (n: number) =>
  Number.isFinite(n) ? n.toLocaleString(BCP47[i18n.locale] ?? BCP47.it) : "—";
