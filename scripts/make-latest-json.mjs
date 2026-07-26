// Costruisce il manifesto `latest.json` che l'updater di Scriptorium interroga.
//
// L'app (tauri-plugin-updater) scarica questo file dall'endpoint configurato in
// tauri.conf.json, confronta `version` con la propria e — se è più nuova —
// scarica l'archivio indicato in `url` verificandone la firma minisign contro la
// chiave pubblica compilata nell'eseguibile. Senza firma valida NON installa.
//
// Uso:  node scripts/make-latest-json.mjs [file-note.md]
// Legge la versione da package.json e la firma dall'artefatto .sig prodotto da
// `tauri build` con TAURI_SIGNING_PRIVATE_KEY_PATH impostata.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const version = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const nsis = join(root, "src-tauri", "target", "release", "bundle", "nsis");
// Tauri 2 firma direttamente l'installer NSIS (niente .nsis.zip: quello era il
// formato della v1). Il plugin scarica questo .exe, ne verifica la firma e lo
// esegue in modalità passiva.
const setup = `Scriptorium_${version}_x64-setup.exe`;
const sigPath = join(nsis, `${setup}.sig`);

if (!existsSync(sigPath)) {
  console.error(
    `Firma mancante: ${sigPath}\n` +
      "Ricompila con TAURI_SIGNING_PRIVATE_KEY e TAURI_SIGNING_PRIVATE_KEY_PASSWORD " +
      'impostate (la seconda vuota se la chiave non ne ha) e "createUpdaterArtifacts": true.',
  );
  process.exit(1);
}

// Le note mostrate nel pannello di aggiornamento: il file passato come argomento
// oppure, in mancanza, la prima sezione del CHANGELOG.
let notes = "";
const notesArg = process.argv[2];
if (notesArg && existsSync(notesArg)) {
  notes = readFileSync(notesArg, "utf8").trim();
} else {
  const md = readFileSync(join(root, "CHANGELOG.md"), "utf8");
  const from = md.indexOf("## ");
  if (from >= 0) {
    const next = md.indexOf("\n## ", from + 3);
    notes = md.slice(from, next < 0 ? undefined : next).trim();
  }
}

const manifest = {
  version,
  notes,
  // Data di pubblicazione in RFC 3339, richiesta dal plugin.
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  platforms: {
    "windows-x86_64": {
      signature: readFileSync(sigPath, "utf8").trim(),
      url: `https://github.com/shelaik/scriptorium/releases/download/v${version}/${setup}`,
    },
  },
};

const out = join(nsis, "latest.json");
writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n", "utf8");
console.log(`latest.json scritto in ${out} (versione ${version})`);
