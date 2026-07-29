// Controllo della traduzione. Tre domande, tre risposte:
//
//  1. MANCANTI  — chiavi usate nel codice e assenti da en.ts: resteranno in
//                 italiano anche con l'interfaccia in inglese.
//  2. ORFANE    — voci di en.ts che nessuno usa piu': una stringa italiana e'
//                 stata modificata e la traduzione e' rimasta indietro. Con «la
//                 stringa italiana e' la chiave» questo non rompe niente, e per
//                 questo va reso rumoroso.
//  3. DIMENTICATE — stringhe italiane ancora NON avvolte in t(). Senza questo
//                 controllo l'estrazione «passa» pur avendo lasciato indietro
//                 meta' dei tooltip e degli aria-label, che sono invisibili a
//                 un collaudo per schermate.
//
// Le chiavi del BACKEND (messaggi scritti in italiano nel Rust e tradotti dal
// frontend con te()) stanno in una sezione marcata di en.ts: non compaiono come
// t("…") nei sorgenti, quindi si verificano contro i sorgenti Rust.
//
// Uso:  node scripts/i18n-check.mjs [--quiet]
// Esce 1 se ci sono chiavi mancanti, segnaposto disallineati o stringhe
// dimenticate; le orfane sono solo un avviso.

import { readFileSync, readdirSync, statSync, existsSync } from "node:fs";
import { join, dirname, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const SRC = join(root, "src");
const RUST = join(root, "src-tauri", "src");
const quiet = process.argv.includes("--quiet");

const BACKEND_MARK = "backend (src-tauri)";
/** Riga marcata cosi' = scelta dichiarata di non tradurre (dato su disco, chiave
 *  di protocollo, valore). Vedi il catalogo in memoria del piano bilingue. */
const EXEMPT = "i18n-exempt";

function walk(dir, ext) {
  const out = [];
  if (!existsSync(dir)) return out;
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) out.push(...walk(p, ext));
    else if (ext.test(name) && !p.includes(join("lib", "i18n"))) out.push(p);
  }
  return out;
}

// ---- 1/2: chiavi usate contro chiavi dichiarate --------------------------
const T_RE = /\bt\(\s*"((?:[^"\\]|\\.)*)"/g;
const TP_RE = /\btp\(\s*[^,]+,\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"/g;

const used = new Map();
const svelteFiles = walk(SRC, /\.(svelte|ts)$/);
for (const file of svelteFiles) {
  const text = readFileSync(file, "utf8");
  const rel = relative(root, file);
  const add = (key) => {
    // Una «chiave» senza lettere non e' testo da tradurre: e' quasi sempre un
    // commento che CITA una chiamata (`t("…")` nella documentazione del codice),
    // che questo estrattore a regex non distingue dal codice vero.
    if (!/[A-Za-zÀ-ÿ]/.test(key)) return;
    if (!used.has(key)) used.set(key, []);
    used.get(key).push(rel);
  };
  let m;
  while ((m = T_RE.exec(text))) add(m[1]);
  while ((m = TP_RE.exec(text))) {
    add(m[1]);
    add(m[2]);
  }
}

const enSrc = readFileSync(join(SRC, "lib", "i18n", "en.ts"), "utf8");
const backendFrom = enSrc.indexOf(BACKEND_MARK);
const declared = new Map(); // chiave -> "ui" | "backend"
const KEY_RE = /^\s*"((?:[^"\\]|\\.)*)"\s*:/gm;
let k;
while ((k = KEY_RE.exec(enSrc))) {
  declared.set(k[1], backendFrom >= 0 && k.index > backendFrom ? "backend" : "ui");
}

// Le stringhe lunghe nel Rust sono spesso spezzate con la continuazione `\` a
// fine riga, che mangia il ritorno a capo E l'indentazione della riga dopo: nel
// sorgente si legge su tre righe, ma la stringa vera e' una sola. Senza questa
// ricomposizione una chiave perfettamente valida risulterebbe «orfana».
const rustSrc = walk(RUST, /\.rs$/)
  .map((f) => readFileSync(f, "utf8"))
  .join("\n")
  .replace(/\\\r?\n[ \t]*/g, "");

// Alcune etichette sono dichiarate in tabelle (temi, modalita' di ricerca,
// criteri d'ordinamento) e tradotte al punto d'uso con `t(x.label)`: la chiave
// e' una stringa letterale nel sorgente, ma non compare mai dentro `t("…")`.
// Cercarle solo come chiamate le farebbe apparire orfane per sempre, e un
// cancello che segnala sempre 18 falsi allarmi e' un cancello che si impara a
// ignorare. Una voce e' orfana quando la sua stringa NON C'E' PIU' da nessuna
// parte nel sorgente — che e' esattamente il caso «testo italiano riscritto,
// traduzione rimasta indietro» che questo controllo deve prendere.
const allSrc = svelteFiles.map((f) => readFileSync(f, "utf8")).join("\n");
const literal = (key) => allSrc.includes(`"${key.replace(/"/g, '\\"')}"`);

const missing = [...used.keys()].filter((x) => !declared.has(x)).sort();
const orphan = [...declared]
  .filter(([key, kind]) =>
    kind === "backend" ? !rustSrc.includes(key) : !used.has(key) && !literal(key),
  )
  .map(([key]) => key)
  .sort();

// Un segnaposto presente da una parte sola e' un messaggio rotto in inglese.
const varsOf = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");
const mismatched = [];
for (const key of declared.keys()) {
  const esc = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const line = enSrc.match(new RegExp(`"${esc}"\\s*:\\s*\\n?\\s*"((?:[^"\\\\]|\\\\.)*)"`));
  if (line && varsOf(key) !== varsOf(line[1])) mismatched.push(key);
}

// ---- 3: stringhe italiane ancora non avvolte -----------------------------
// Gli attributi sono il punto cieco: non si vedono in una schermata e sono ~440.
const ATTR_RE = /\b(title|aria-label|placeholder|alt|aria-description)="([^"{}]{2,})"/g;
const forgotten = [];
for (const file of svelteFiles) {
  if (!file.endsWith(".svelte")) continue;
  const rel = relative(root, file);
  const lines = readFileSync(file, "utf8").split("\n");
  lines.forEach((line, i) => {
    if (line.includes(EXEMPT)) return;
    let m;
    ATTR_RE.lastIndex = 0;
    while ((m = ATTR_RE.exec(line))) {
      const v = m[2].trim();
      // Solo testo umano: almeno due lettere e uno spazio o un accento — cosi'
      // restano fuori i valori tecnici (`button`, `polite`, `1`, `#fff`).
      if (!/[A-Za-zÀ-ÿ]{2}/.test(v)) continue;
      if (!/\s|[À-ÿ]/.test(v)) continue;
      forgotten.push(`${rel}:${i + 1}  ${m[1]}="${v.slice(0, 60)}"`);
    }
  });
}

const ui = [...declared.values()].filter((v) => v === "ui").length;
console.log(`chiavi usate nel sorgente:   ${used.size}`);
console.log(`chiavi in en.ts:             ${declared.size} (${ui} interfaccia, ${declared.size - ui} backend)`);
console.log(`da tradurre (mancanti):      ${missing.length}`);
console.log(`orfane (traduzione stale):   ${orphan.length}`);
console.log(`segnaposto disallineati:     ${mismatched.length}`);
console.log(`attributi non avvolti:       ${forgotten.length}`);

if (!quiet) {
  const show = (title, list, n = 30) => {
    if (!list.length) return;
    console.log(`\n--- ${title} (${list.length}) ---`);
    for (const x of list.slice(0, n)) console.log(`  ${typeof x === "string" && x.includes(":") ? x : JSON.stringify(x)}`);
    if (list.length > n) console.log(`  …e altre ${list.length - n}`);
  };
  show("DA TRADURRE", missing);
  show("ORFANE", orphan);
  show("SEGNAPOSTO DISALLINEATI", mismatched);
  show("ATTRIBUTI NON AVVOLTI", forgotten);
}

process.exit(missing.length || mismatched.length || forgotten.length ? 1 : 0);
