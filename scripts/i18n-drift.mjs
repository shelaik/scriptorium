// Verifica la rete di sicurezza dell'estrazione: **in italiano l'app deve
// restare identica**.
//
// Poiche' la chiave e' la stringa italiana e in italiano `t(x)` restituisce x,
// ogni chiave che compare nel codice nuovo dovrebbe esistere — parola per
// parola — anche nel codice PRIMA dell'estrazione. Se non c'e', il testo
// italiano mostrato all'utente e' cambiato: o e' una riscrittura voluta (le
// frasi montate a pezzi, che non erano traducibili), o e' un errore silenzioso
// che nessun compilatore puo' vedere.
//
// Uso:  node scripts/i18n-drift.mjs [ref-git]     (default: la revisione prima
//       dell'estrazione, passata come argomento — es. HEAD)
//
// I segnaposto `{nome}` introdotti al posto di `${...}` non possono combaciare
// alla lettera: la ricerca li tratta come jolly.

import { execSync } from "node:child_process";
import { readdirSync, statSync, readFileSync } from "node:fs";
import { join, dirname, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const ref = process.argv[2] || "HEAD";

function walk(dir) {
  const out = [];
  for (const n of readdirSync(dir)) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) out.push(...walk(p));
    else if (/\.(svelte|ts)$/.test(n) && !p.includes(join("lib", "i18n"))) out.push(p);
  }
  return out;
}

// Il sorgente com'era PRIMA, letto da git (un solo blob concatenato: la stringa
// puo' essersi spostata di file, e non e' quello che stiamo verificando).
const files = walk(join(root, "src")).map((f) => relative(root, f).replace(/\\/g, "/"));
let before = "";
for (const f of files) {
  try {
    before += execSync(`git show ${ref}:${f}`, { cwd: root, encoding: "utf8", maxBuffer: 1e8 });
  } catch {
    /* file nuovo: non esisteva prima, nulla da confrontare */
  }
  before += "\n";
}

const T_RE = /\bt\(\s*"((?:[^"\\]|\\.)*)"/g;
const TP_RE = /\btp\(\s*[^,]+,\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"/g;

const keys = new Map();
for (const f of files) {
  const text = readFileSync(join(root, f), "utf8");
  const add = (k) => {
    if (!/[A-Za-zÀ-ÿ]/.test(k)) return;
    if (!keys.has(k)) keys.set(k, f);
  };
  let m;
  while ((m = T_RE.exec(text))) add(m[1]);
  while ((m = TP_RE.exec(text))) {
    add(m[1]);
    add(m[2]);
  }
}

const esc = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
/** La chiave come regex: i segnaposto diventano jolly, perche' prima erano
 *  interpolazioni `${...}` di forma qualunque. */
function pattern(key) {
  return new RegExp(
    key
      .split(/\{\w+\}/)
      .map(esc)
      .join("[\\s\\S]{0,120}"),
  );
}

const drifted = [];
for (const [key, file] of keys) {
  // Le chiavi con contesto (`Nota|del documento`) mostrano solo la parte prima.
  const plain = key.lastIndexOf("|") > 0 ? key.slice(0, key.lastIndexOf("|")) : key;
  if (before.includes(plain)) continue;
  if (pattern(plain).test(before)) continue;
  drifted.push({ key, file });
}

console.log(`chiavi verificate:        ${keys.size}`);
console.log(`presenti già prima:       ${keys.size - drifted.length}`);
console.log(`TESTO ITALIANO CAMBIATO:  ${drifted.length}`);
if (drifted.length) {
  console.log("\n--- da rileggere una per una: o è una riscrittura voluta, o è un errore ---");
  const limit = process.argv.includes("--all") ? drifted.length : 60;
  for (const d of drifted.slice(0, limit)) console.log(`  [${d.file}] ${JSON.stringify(d.key)}`);
  if (drifted.length > limit) console.log(`  …e altre ${drifted.length - limit} (rilancia con --all)`);
}
