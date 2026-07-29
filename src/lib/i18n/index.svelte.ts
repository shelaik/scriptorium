// Bilinguismo italiano / inglese.
//
// SCELTA PORTANTE: **la stringa italiana e' la chiave**. Esiste un solo
// dizionario (`en.ts`); in italiano `t()` e' la funzione identita'.
//
// Tre conseguenze volute:
//  1. l'app in italiano resta DIMOSTRABILMENTE identica — se dopo l'estrazione
//     funziona ancora in italiano, l'estrazione non ha cambiato semantica;
//  2. una traduzione mancante degrada in italiano, mai in `missing.key.name`
//     mostrato all'utente;
//  3. non c'e' un secondo dizionario da tenere allineato, quindi non c'e' la
//     classe di errori «chiave rinominata di qua e non di la'».
//
// Il prezzo: modificare una stringa italiana rende orfana la sua traduzione, in
// silenzio. Lo paga `scripts/i18n-check.mjs`, che confronta le chiavi trovate
// nel sorgente con quelle di `en.ts` e segnala mancanti e orfane; sta nel ciclo
// di rilascio (RELEASING.md).
//
// Niente librerie: la CSP dell'app (`script-src 'self' 'wasm-unsafe-eval'`, senza
// `unsafe-eval`) uccide qualunque runtime i18n che compili i messaggi con
// `new Function`.

import { EN } from "./en";

export type Locale = "it" | "en";

export const LOCALES: { value: Locale; label: string }[] = [
  { value: "it", label: "Italiano" },
  { value: "en", label: "English" },
];

const STORE_KEY = "scriptorium-locale";

function isLocale(v: unknown): v is Locale {
  return v === "it" || v === "en";
}

/** Lingua all'avvio: la scelta esplicita vince SEMPRE, e in sua assenza si parte
 *  in ITALIANO.
 *
 *  Non si guarda la lingua di Windows qui, di proposito. Scriptorium e' stato
 *  italiano per cinquanta versioni: chi lo aggiorna non ha una scelta salvata, e
 *  su un Windows in inglese si vedrebbe l'interfaccia cambiare lingua da sola,
 *  senza aver toccato niente. La lingua di sistema vale solo come primo valore di
 *  un'installazione DAVVERO nuova, e quel caso lo decide `systemLocale()`, che il
 *  frontend usa solo quando il database dice che non c'e' mai stata una versione
 *  precedente.
 *
 *  Tutto sotto try: questo modulo viene eseguito anche in Node durante la
 *  prerenderizzazione della finestra Plancia, dove `localStorage` e `navigator`
 *  non esistono. */
function detect(): Locale {
  try {
    const saved = localStorage.getItem(STORE_KEY);
    if (isLocale(saved)) return saved;
  } catch {
    /* niente storage: italiano */
  }
  return "it";
}

/** La lingua di sistema, se e' una di quelle che parliamo. Solo per la primissima
 *  apertura di un'installazione nuova. */
export function systemLocale(): Locale | null {
  try {
    const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
    for (const l of langs) {
      const base = String(l || "").slice(0, 2).toLowerCase();
      if (base === "it") return "it";
      if (base === "en") return "en";
    }
  } catch {
    /* niente navigator (prerender) */
  }
  return null;
}

/** Vero se l'utente ha gia' scelto esplicitamente una lingua. */
export function hasExplicitLocale(): boolean {
  try {
    return isLocale(localStorage.getItem(STORE_KEY));
  } catch {
    return false;
  }
}

let current = $state<Locale>(detect());

// La finestra Plancia e' una webview SEPARATA: ha una sua copia di questo
// modulo, quindi il suo `current` non sa nulla della scelta fatta nella finestra
// principale e resterebbe nella lingua di apertura fino a chiusura e riapertura.
// L'evento `storage` scatta proprio nelle ALTRE finestre della stessa origine
// quando localStorage cambia: e' il canale che le tiene allineate, senza
// aggiungere un evento Tauri e senza che le due finestre si conoscano.
try {
  window.addEventListener("storage", (e) => {
    if (e.key === STORE_KEY && isLocale(e.newValue) && e.newValue !== current) {
      current = e.newValue;
      try {
        document.documentElement.lang = e.newValue;
      } catch {
        /* nessun documento */
      }
    }
  });
} catch {
  /* nessuna window (prerender in Node) */
}

/** Lo stato non si puo' esportare direttamente: Svelte 5 rifiuta di compilare
 *  `export let x = $state(...)` se x viene riassegnata da un altro modulo.
 *  Un oggetto con getter/setter e' il modo previsto. */
export const i18n = {
  get locale(): Locale {
    return current;
  },
  set locale(v: Locale) {
    if (!isLocale(v) || v === current) return;
    current = v;
    try {
      localStorage.setItem(STORE_KEY, v);
    } catch {
      /* la scelta vale comunque per questa sessione */
    }
    try {
      document.documentElement.lang = v;
    } catch {
      /* nessun documento (prerender) */
    }
  },
};

/** Allinea `<html lang>` alla lingua corrente; da chiamare una volta all'avvio. */
export function applyLangAttribute(): void {
  try {
    document.documentElement.lang = current;
  } catch {
    /* nessun documento */
  }
}

export type Vars = Record<string, string | number>;

/** Sostituisce i segnaposto `{nome}`. Un segnaposto senza valore resta com'e':
 *  meglio un `{n}` visibile in un messaggio che un `undefined`. */
function fill(s: string, vars?: Vars): string {
  if (!vars) return s;
  return s.replace(/\{(\w+)\}/g, (m, k: string) => (k in vars ? String(vars[k]) : m));
}

/** Toglie il suffisso di disambiguazione: `"Nota|del documento"` -> `"Nota"`.
 *  Serve quando la stessa parola italiana ha due traduzioni inglesi diverse; le
 *  stringhe dell'interfaccia non contengono barre verticali, quindi e' sicuro. */
function strip(key: string): string {
  const i = key.lastIndexOf("|");
  return i > 0 ? key.slice(0, i) : key;
}

/** Traduce. In italiano restituisce la chiave stessa (senza il contesto). */
export function t(key: string, vars?: Vars): string {
  const s = current === "it" ? strip(key) : (EN[key] ?? strip(key));
  return fill(s, vars);
}

/** Plurale a due forme (sufficiente per italiano e inglese). Entrambe le forme
 *  italiane sono chiavi del dizionario, cosi' l'inglese puo' scegliere parole
 *  diverse e non solo una «s» finale. */
export function tp(n: number, one: string, other: string, vars?: Vars): string {
  return t(n === 1 ? one : other, { n, ...(vars ?? {}) });
}

/** Vero quando l'interfaccia e' in inglese: per i rari punti in cui la lingua
 *  cambia la logica e non solo il testo (formattatori, prompt AI). */
export function isEnglish(): boolean {
  return current === "en";
}

/** Marcatore del backend per «il PDF non e' piu' al percorso salvato»: e' un
 *  token di protocollo, non una frase, e non va MAI toccato da `te()`. */
const MISSING_FILE_MARKER = "FILE_MANCANTE:";

/** Traduce un messaggio ARRIVATO DAL BACKEND.
 *
 *  Il Rust resta interamente in italiano e non conosce la lingua scelta: e' il
 *  consumatore a tradurre. Il vantaggio e' che il registro diagnostico su disco
 *  (`logs/plancia-*.jsonl`) e l'export restano in una lingua sola, e che il
 *  backend non deve portarsi dietro un secondo dizionario.
 *
 *  Due regole, in quest'ordine:
 *   1. corrispondenza esatta — copre i messaggi scritti per intero nel Rust;
 *   2. corrispondenza per SEGMENTI separati da ": " — recupera le catene tipo
 *      «Contesto italiano: causa» prodotte da anyhow, traducendo i segmenti che
 *      conosce e lasciando intatti gli altri (che sono la causa tecnica, spesso
 *      gia' inglese e non nostra).
 *
 *  La regola 2 puo' in teoria produrre una frase meta' tradotta; e' accettabile
 *  perche' il segmento non tradotto e' quasi sempre il dettaglio tecnico, ed e'
 *  il motivo per cui `scripts/i18n-check.mjs` deve elencare le chiavi di backend
 *  come tutte le altre: una chiave dimenticata qui si vede. */
export function te(msg: string): string {
  if (current === "it" || !msg) return msg;
  // Il marcatore e' un contratto fra Rust e lettore: tradurlo lo romperebbe.
  if (msg.includes(MISSING_FILE_MARKER)) return msg;
  const whole = EN[msg];
  if (whole) return whole;
  if (!msg.includes(": ")) return msg;
  return msg
    .split(": ")
    .map((seg) => EN[seg] ?? seg)
    .join(": ");
}
