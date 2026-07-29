// Date, time and number formatting for the whole app, in ONE place.
//
// Prima ogni punto chiamava `toLocaleDateString("it-IT", …)` col codice lingua
// scritto a mano: cinque copie sparse fra la pagina principale, i progetti
// LaTeX e la Plancia. Quando l'interfaccia diventera' bilingue quel codice
// dovra' cambiare insieme alla lingua scelta, e cercarlo file per file e'
// esattamente il modo in cui se ne dimentica uno.
//
// Qui c'e' un solo punto da cambiare: `APP_LOCALE`. Il resto dell'app chiama i
// tre helper e non sa nulla di codici lingua.

/** BCP-47 usato da tutti i formattatori. Diventera' reattivo con la lingua scelta. */
export const APP_LOCALE = "it-IT";

/** «10 lug 2026» — data compatta, per elenchi fitti. */
const shortDate = new Intl.DateTimeFormat(APP_LOCALE, {
  day: "numeric",
  month: "short",
  year: "numeric",
});

/** «10 lug 2026, 14:32» — data e ora, per le righe di dettaglio. */
const dateTime = new Intl.DateTimeFormat(APP_LOCALE, {
  dateStyle: "medium",
  timeStyle: "short",
});

/** «10 lug» — giorno e mese, quando l'anno e' implicito nel contesto. */
const dayMonth = new Intl.DateTimeFormat(APP_LOCALE, { day: "numeric", month: "short" });

/** Orologio a 24 ore. `hour12` e' esplicito di proposito: il valore predefinito
 *  dipende dalla lingua (l'inglese americano passerebbe a 12 ore + AM/PM) e il
 *  registro della Plancia dev'essere leggibile allo stesso modo ovunque. */
const clock = new Intl.DateTimeFormat(APP_LOCALE, {
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

/** Un formattatore non deve MAI far saltare una vista: timestamp assurdi o
 *  `Invalid Date` tornano come trattino. */
function safe(fmt: Intl.DateTimeFormat, ms: number | null | undefined): string {
  if (ms == null || !Number.isFinite(ms)) return "—";
  try {
    return fmt.format(new Date(ms));
  } catch {
    return "—";
  }
}

export const fmtDateShort = (ms: number | null | undefined) => safe(shortDate, ms);
export const fmtDateTime = (ms: number | null | undefined) => safe(dateTime, ms);
export const fmtDayMonth = (ms: number | null | undefined) => safe(dayMonth, ms);
export const fmtClock = (ms: number | null | undefined) => safe(clock, ms);

/** Numeri col separatore delle migliaia della lingua corrente. */
export const fmtNum = (n: number) => (Number.isFinite(n) ? n.toLocaleString(APP_LOCALE) : "—");
