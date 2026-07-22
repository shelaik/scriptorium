# Scriptorium

**Gestore locale di PDF scientifici, riferimenti e appunti** — veloce, privato, senza cloud.
*(A lean, local-first PDF & reference manager for research papers, with local-AI features. Italian UI.)*

Tutto vive sul tuo computer: i PDF, il catalogo (SQLite), gli appunti (file `.md` veri), i progetti LaTeX (cartelle vere). Le funzioni di rete (ricerca online, metadati) e l'AI locale (Ollama / LM Studio) sono **opzionali e disattivabili**.

## Cosa fa

- **Libreria**: import da disco/BibTeX/DOI/arXiv/URL/zip LaTeX, cartella sorvegliata, aggancio dal browser; tag, collezioni smart, filtri, metadati precision-first (mai un'etichetta incerta) con recupero in blocco e per scheda (candidati multi-fonte da confermare); salute libreria, gap di citazioni, duplicati, backup.
- **Lettura**: lettore immersivo con annotazioni ancorate, ricerca nel documento, e **estrazione**: tabelle (motore nativo, modello TATR locale, o vision-LLM), testo con formattazione (corsivi/apici/pedici → Markdown), **formule → LaTeX** (math-OCR locale Pix2Text-MFR con anteprima modificabile), figure → PNG.
- **Scrittura**: **Appunti** in Markdown (file veri, `[[wikilink]]` verso paper e appunti, backlink, formule KaTeX, immagini, export MD/HTML/LaTeX/PDF); **Wiki della libreria** (enciclopedia privata generata dall'AI locale sui *tuoi* documenti, citazioni che aprono il PDF alla pagina giusta); **Progetti LaTeX** (un piccolo Overleaf locale: modelli, `\cite{}` dalla libreria, `refs.bib` sincronizzato, compilazione con Tectonic/MiKTeX e anteprima in-app).
- **Scoperta**: ricerca su arXiv/OpenAlex/ADS/Semantic Scholar/…, ricerche salvate con campana **Novità**, mappa delle citazioni, **Costellazione** (la libreria come grafo semantico: comunità, cerca-nel-grafo, stelle fantasma da OpenAlex con **esplorazione a catena** dalle scoperte stesse, appunti nel grafo).
- **AI locale** (opzionale): riassunti, tag automatici, lente di lettura, «Chiedi alla libreria» con fonti, confronti e rassegne. Nessun dato esce dal PC.
- **Interfaccia «Orbita»**: barra strumenti + **menu radiale** (tasto destro) + **palette comandi** (Ctrl+K) che pescano dallo stesso registro; guida integrata a schede con FAQ, in finestra flottante.
- **Da fuori**: CLI (`scriptorium-cli`) e **server MCP** (`scriptorium-mcp`) read-only per terminale, script, Claude Desktop/Code — vedi la sezione dedicata.

## Installazione

Scarica l'ultimo `Scriptorium_x.y.z_x64-setup.exe` dalle **[Release](https://github.com/shelaik/scriptorium/releases)** ed eseguilo (Windows 10/11 x64). Ogni versione pubblicata corrisponde al commit «Release …» omonimo; in alternativa puoi compilare dal sorgente (sotto).

## Stack

Tauri 2 · Rust (rusqlite + FTS5 + sqlite-vec, pdfium, ONNX Runtime per embedding bge-m3 / math-OCR / TATR) · SvelteKit + Svelte 5 (runes) · KaTeX · pdf.js. Solo Windows al momento. Attribuzioni dei componenti terzi in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

## Build

```
npm install
npm run tauri build     # installer NSIS in src-tauri/target/release/bundle/nsis/
npm run tauri dev       # sviluppo
```

Requisiti: Node 20+, Rust stable, WebView2. I modelli locali (embedding, math-OCR, struttura tabelle) si scaricano al primo uso in `%APPDATA%\com.pdfmanage.app\`. Per compilare i progetti LaTeX serve un toolchain di sistema (Tectonic consigliato: `winget install Tectonic.Tectonic`, oppure MiKTeX/TeX Live).

## Dati

`%APPDATA%\com.pdfmanage.app\` → `pdfmanage.db` (catalogo), `papers/` (PDF scaricati; quelli importati dal disco restano dove sono), `notes/` (appunti .md), `projects/` (LaTeX), `backups/` (copie automatiche del DB a ogni aggiornamento di versione), modelli locali.

## CLI e server MCP

Due binari compagni, **read-only** e sicuri da usare mentre l'app è aperta (il DB è SQLite in WAL). Entrambi sono allegati alle **Release**; percorsi e configurazione pronti da copiare in **Impostazioni → CLI e MCP**.

**`scriptorium-cli`** — interroga libreria, Appunti e progetti LaTeX da terminale, output JSON (comodo per script e Claude Code): `query`, `list`, `show`, `tags`, `stats`, `bib` (BibTeX), `notes` / `note <slug>` / `search-notes` (il vault .md), `projects`, `schema`, `version`.

**`scriptorium-mcp`** — server **MCP** locale (stdio, niente porte né processi in background: lo avvia il client quando serve) che porta gli stessi dati dentro **Claude Desktop / Claude Code** e qualsiasi client MCP, con 9 strumenti: `search_library`, `list_documents`, `get_document`, `get_bibtex`, `list_notes`, `get_note`, `search_notes`, `list_projects`, `library_stats`. Registrazione in Claude Code:

```
claude mcp add scriptorium -- "%LOCALAPPDATA%\Scriptorium\scriptorium-mcp.exe"
```

(in Claude Desktop: voce `"scriptorium": { "command": "<percorso>\\scriptorium-mcp.exe" }` sotto `mcpServers`).

Compilazione dal sorgente (da `src-tauri/`):

```
cargo build --release --bin scriptorium-cli --features cli
cargo build --release --bin scriptorium-mcp --features mcp
```

## Note

- Progetto personale in italiano; changelog in [CHANGELOG.md](CHANGELOG.md).
- Il controllo aggiornamenti in-app è un semplice avviso (legge il `package.json` di questo repo, se pubblico); niente auto-update.
