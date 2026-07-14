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

## Installazione

Scarica l'ultimo `Scriptorium_x.y.z_x64-setup.exe` dalle **[Release](https://github.com/shelaik/scriptorium/releases)** ed eseguilo (Windows 10/11 x64). Ogni versione pubblicata corrisponde al commit «Release …» omonimo; in alternativa puoi compilare dal sorgente (sotto).

## Stack

Tauri 2 · Rust (rusqlite + FTS5 + sqlite-vec, pdfium, ONNX Runtime per embedding bge-m3 / math-OCR / TATR) · SvelteKit + Svelte 5 (runes) · KaTeX · pdf.js. Solo Windows al momento.

## Build

```
npm install
npm run tauri build     # installer NSIS in src-tauri/target/release/bundle/nsis/
npm run tauri dev       # sviluppo
```

Requisiti: Node 20+, Rust stable, WebView2. I modelli locali (embedding, math-OCR, struttura tabelle) si scaricano al primo uso in `%APPDATA%\com.pdfmanage.app\`. Per compilare i progetti LaTeX serve un toolchain di sistema (Tectonic consigliato: `winget install Tectonic.Tectonic`, oppure MiKTeX/TeX Live).

## Dati

`%APPDATA%\com.pdfmanage.app\` → `pdfmanage.db` (catalogo), `papers/` (PDF scaricati; quelli importati dal disco restano dove sono), `notes/` (appunti .md), `projects/` (LaTeX), `backups/` (copie automatiche del DB a ogni aggiornamento di versione), modelli locali.

## CLI

`scriptorium-cli` è un binario separato, **read-only** e sicuro da usare mentre l'app è aperta (il DB è SQLite in WAL): interroga libreria, Appunti e progetti LaTeX da terminale — comodo per script e per Claude Code. Comandi: `query`, `list`, `show`, `tags`, `stats`, `bib` (BibTeX), `notes` / `note <slug>` / `search-notes` (il vault .md), `projects`, `schema`. Output JSON. Si compila con:

```
cargo build --release --bin scriptorium-cli --features cli   # da src-tauri/
```

(oppure scarica `scriptorium-cli.exe` dalle Release, quando allegato).

## Note

- Progetto personale in italiano; changelog in [CHANGELOG.md](CHANGELOG.md).
- Il controllo aggiornamenti in-app è un semplice avviso (legge il `package.json` di questo repo, se pubblico); niente auto-update.
