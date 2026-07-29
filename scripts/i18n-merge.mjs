// Fonde i frammenti di dizionario prodotti dall'estrazione (un file JSON per
// sorgente) dentro `src/lib/i18n/en.ts`, in ordine e senza duplicati.
//
// Serve perche' l'estrazione avviene in parallelo su file diversi: se ogni
// estrattore scrivesse direttamente in en.ts si sovrascriverebbero a vicenda.
// Ognuno deposita il suo frammento, questo script li cuce.
//
// Uso:  node scripts/i18n-merge.mjs <cartella-dei-frammenti>
// Idempotente: rilanciarlo con gli stessi frammenti non cambia il risultato.
// Le voci gia' presenti in en.ts NON vengono sovrascritte (la traduzione
// rivista a mano vince su quella rigenerata).

import { readFileSync, writeFileSync, readdirSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const EN = join(root, "src", "lib", "i18n", "en.ts");
const partsDir = process.argv[2];

if (!partsDir || !existsSync(partsDir)) {
  console.error("Uso: node scripts/i18n-merge.mjs <cartella-dei-frammenti>");
  process.exit(1);
}

const src = readFileSync(EN, "utf8");
const head = src.slice(0, src.indexOf("export const EN"));

// Le voci gia' dichiarate, per non perderne nessuna e non sovrascriverle.
// Si parte DOPO `export const EN`: l'intestazione del file contiene esempi di
// voci dentro i commenti (`"Nota|del documento": "Note"`) e leggerli come voci
// vere le resuscita a ogni fusione — cosa che e' successa davvero.
const existing = new Map();
const body = src.slice(src.indexOf("export const EN"));
const ENTRY_RE = /"((?:[^"\\]|\\.)*)"\s*:\s*\n?\s*"((?:[^"\\]|\\.)*)"/g;
let m;
while ((m = ENTRY_RE.exec(body))) existing.set(unesc(m[1]), unesc(m[2]));

function unesc(s) {
  return s.replace(/\\(["\\nrt])/g, (_, c) => ({ '"': '"', "\\": "\\", n: "\n", r: "\r", t: "\t" })[c]);
}
function esc(s) {
  return s.replace(/\\/g, "\\\\").replace(/"/g, '\\"').replace(/\n/g, "\\n");
}

let added = 0;
let kept = 0;
const bySource = [];
for (const name of readdirSync(partsDir).filter((f) => f.endsWith(".json")).sort()) {
  const obj = JSON.parse(readFileSync(join(partsDir, name), "utf8"));
  const rows = [];
  for (const [k, v] of Object.entries(obj)) {
    if (typeof v !== "string" || !k) continue;
    if (existing.has(k)) {
      kept++;
      continue;
    }
    existing.set(k, v);
    rows.push([k, v]);
    added++;
  }
  if (rows.length) bySource.push([name.replace(/\.json$/, ""), rows]);
}

// Riscrive il file intero: prima le voci gia' presenti (nell'ordine in cui
// erano), poi i nuovi blocchi raggruppati per sorgente, cosi' il diff e' leggibile.
const already = [...existing.keys()].filter((k) => !bySource.some(([, rows]) => rows.some(([kk]) => kk === k)));

const out = [];
out.push(head);
out.push("export const EN: Record<string, string> = {");
if (already.length) {
  for (const k of already) out.push(`  "${esc(k)}":\n    "${esc(existing.get(k))}",`);
}
for (const [source, rows] of bySource) {
  out.push(`\n  // ---- ${source} ----`);
  for (const [k, v] of rows) out.push(`  "${esc(k)}":\n    "${esc(v)}",`);
}
out.push("};\n");

writeFileSync(EN, out.join("\n"), "utf8");
console.log(`frammenti fusi: ${bySource.length}`);
console.log(`voci nuove:     ${added}`);
console.log(`voci gia' presenti (non toccate): ${kept}`);
console.log(`totale in en.ts: ${existing.size}`);
