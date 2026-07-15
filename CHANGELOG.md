# Changelog

Rilasci principali di Scriptorium. Ogni versione è nel messaggio del commit «Release …» corrispondente; qui il sunto.

## 0.9.24 — Prestazioni libreria grande + reimport dal Cestino
- **Griglia più leggera**: le schede fuori schermo non vengono più disegnate finché non ti avvicini (grazie a `content-visibility`) — scorrimento e primo caricamento fluidi anche con migliaia di documenti.
- **Meno lavoro sul database a ogni aggiornamento**: autori e tag di tutta la libreria si leggono ora in poche query in blocco invece di due per ogni documento (era un classico «N+1» che rallentava import, tag, preferiti, cambio filtro man mano che la libreria cresce).
- **Reimport dal Cestino più intelligente**: reimportare (trascinamento / file) un paper che è nel Cestino ora lo **ripristina con tutto** (tag, note, annotazioni) anche se lo riprendi da una **cartella diversa** — prima solo dallo stesso percorso; se ne trovavi una copia altrove restava un «duplicato» invisibile. La riscansione della cartella sorvegliata continua a non ripristinare nulla da sola (il Cestino resta il Cestino).

## 0.9.23 — Irrobustimento: niente crash pdfium, backup coerenti, link sicuri
Giro di consolidamento dopo un audit interno (24 punti deboli confermati e corretti). Nessuna funzione nuova: robustezza, integrità dei dati e sicurezza.
- **Niente più crash nativo (0xc0000409)**: sei operazioni pdfium (indice RAG, rigenerazione anteprine, OCR, estrazione tabelle/testo di una regione) non prendevano il lock che serializza il lavoro su tutto il documento; ora sì. Prima potevano andare in conflitto con un import (cartella sorvegliata o manuale) e far chiudere l'app di colpo.
- **Backup della libreria coerente e non bloccante**: la copia ora scatta un checkpoint del WAL e copia il database sotto lock (istantanea a un solo istante, mai una coppia `.db`/`.db-wal` disallineata) e gira fuori dal thread dell'interfaccia (niente più «Non risponde» su librerie grandi). Il backup pre-migrazione all'avvio segna la versione **solo se la copia è riuscita** (disco pieno / file bloccato → riprova al prossimo avvio invece di saltare la rete di sicurezza).
- **Link esterni più sicuri**: `apri nel browser` ora valida l'host con lo stesso parser del browser (WHATWG), chiudendo un aggiramento con `\` (es. `http://127.0.0.1\@host/`) che poteva puntare a `localhost`/rete locale.
- **Cestino coerente**: reimportare o «Trova PDF» di un paper che è nel Cestino ora lo **ripristina** invece di dare un errore `UNIQUE` o un «duplicato» invisibile; svuotamento del Cestino e azzeramento indici avvengono in transazione unica (niente stati a metà se manca la corrente); niente più schede-fantasma quando si ri-aggiunge un paper il cui DOI era su una scheda cestinata.
- **Più fluido**: anteprime caricate a piccoli gruppi invece di migliaia di chiamate in blocco all'avvio (e ripulite quando svuoti il Cestino); la mappa (Costellazione) **smette di ridisegnarsi** quando apri un PDF che la copre; ricerca in libreria che non lascia più lo spinner «cerco…» bloccato se svuoti la casella a metà.
- **Guida allineata**: corrette alcune diciture non più vere (menu Sistema, «Verifica e ripara metadati», modalità di ricerca «Tutto», «Cerca online», toggle delle funzioni online).

## 0.9.22 — Server MCP
- Nuovo binario **`scriptorium-mcp`**: server **MCP** locale (stdio, read-only, gemello della CLI) che porta la libreria dentro **Claude Desktop / Claude Code** e qualsiasi client MCP — 9 strumenti: `search_library`, `list_documents`, `get_document`, `get_bibtex`, `list_notes`, `get_note`, `search_notes`, `list_projects`, `library_stats`. Nessun servizio in background: lo avvia il client quando serve. Allegato alle Release.
- Nuova scheda **Impostazioni → CLI e MCP**: percorsi dei binari con verifica di presenza e configurazione pronta da copiare (comando `claude mcp add` e voce per `claude_desktop_config.json`).
- README riscritto nella sezione CLI/MCP.

## 0.9.21 — Esplorazione: i nuclei si staccano + fantasmi trascinabili
- Quando esplori **da** una scoperta, quella stella (con tutto il suo ventaglio) ora **si stacca** dalla stella d'origine: si sposta ad almeno **1,5× il raggio massimo del proprio ventaglio**, nella **direzione più libera** attorno alla base (24 direzioni campionate, evitando stelle della libreria e altre catene, con preferenza verso l'esterno). Le catene di hub si distanziano a cascata: niente più nuclei impilati.
- Le **stelle fantasma si trascinano**: sposti una scoperta (o un intero nucleo: la sua catena la segue con le molle) e resta dove la lasci. Nessuna funzione o grafica esistente è cambiata.

## 0.9.20 — Modalità esplorazione nel grafo + citazioni senza DOI
- **Modalità esplorazione**: mentre ci sono stelle fantasma la Costellazione cambia pelle — la libreria si attenua a fondale, i paper di partenza hanno un anello «scanner» rotante, ogni **hop** della catena ha il suo colore, i collegamenti sono **archi curvi animati** e le scoperte pulsano con un alone. Le catene multi-hop si dispongono da sole: i fantasmi hanno una **mini-fisica** dedicata (si respingono tra loro e stanno alla larga dalle stelle della libreria), quindi niente più sovrapposizioni. La funzionalità esistente non cambia: × per uscire e la mappa torna com'era.
- **Citazioni «da e per» senza DOI**: l'esplorazione delle citazioni ora aggancia il paper anche per **id OpenAlex** (le scoperte ce l'hanno sempre: mai più «manca il DOI» sui fantasmi) o per **titolo** con corrispondenza rigorosa (per i paper della libreria senza DOI). Vale nel grafo, nel radiale («Esplora citazioni (online)») e nella finestra Esplora citazioni («Esplora da qui» / «Esplora ↗» sempre disponibili).

## 0.9.19 — CLI estesa + esplorazione a catena dalle stelle fantasma
- **CLI `scriptorium-cli` aggiornata** (era ferma alla libreria): nuovi comandi `notes` (elenco del vault Appunti .md), `note <slug>` (stampa il Markdown), `search-notes` (ricerca con estratto), `projects` (progetti LaTeX), `version`; `schema` documenta anche i percorsi dei file. Sempre read-only e sicura con l'app aperta; binario allegato alla release.
- **Costellazione — esplorazione a catena**: anche la scheda di una **stella fantasma** ha Citazioni / Simili / Autore; le nuove scoperte si agganciano a quella (con la linea tratteggiata al genitore), così si scava di paper in paper senza dover aggiungere nulla.
- README aggiornato (sezione CLI + Scoperta).

## 0.9.18 — Fix: suggerimenti della ricerca nel grafo
- Corretto il menù dei candidati di «Cerca nel grafo…»: una regola CSS della barra (`.hud button`, più specifica) schiacciava ogni riga in un quadrato 26×26 col testo che traboccava dal riquadro. Ora le righe sono normali (titoli su due righe, anno a destra) e anche i titoli-nomefile senza spazi vanno a capo. Diagnosi fatta su una pagina di riproduzione renderizzata headless.

## 0.9.17 — Trova PDF con candidati
- **«Trova PDF…» ora mostra i candidati** invece di fallire in silenzio: cerca per identificativo (arXiv, DOI → Unpaywall/OpenAlex/Semantic Scholar) e **per titolo** su arXiv, OpenAlex, Semantic Scholar e Crossref; ogni candidato arriva con le prove (titolo identico/simile, autori coincidenti, anno) e i pulsanti **«Scarica e allega»** e **«Apri pagina»** (per controllare nel browser). In fondo, il campo per allegare un link diretto. Se un candidato non è scaricabile, la finestra resta aperta e provi il prossimo.
- Il recupero **automatico** (selezione multipla e in blocco) ora cerca per titolo anche su **arXiv** e **Semantic Scholar** (sempre col gate rigoroso): il caso «lo trovavo a mano su arXiv» ora lo trova da solo.

## 0.9.16 — Trova PDF robusto + rifiniture ricerca grafo + fix esplora citazioni
- **«Trova PDF» in cascata** per le voci solo-riferimento: arXiv (id o DOI), Unpaywall (se c'è l'email, non più obbligatoria), OpenAlex, Semantic Scholar, e ricerca per titolo con gate rigoroso se manca il DOI; se un link OA è morto si passa al successivo. Ora anche nel **radiale della scheda** («Trova PDF»), sulla **selezione multipla** e in blocco su tutta la libreria (Cura della libreria → «Trova PDF dei riferimenti»), con barra di avanzamento e Stop.
- **Ricerca nel grafo**: menù dei candidati più largo (i titoli lunghi vanno su due righe), niente scroll orizzontale, scrollbar verticale sottile ed elegante.
- **Esplora citazioni**: il riquadro delle opzioni che si apre cliccando un nodo compariva **dietro** la finestra — ora sta sopra (e più largo).

## 0.9.15 — Cerca nella Costellazione
- **Casella «Cerca nel grafo…»** nella HUD della vista a grafo (accanto a Colora per / Nebulose): da 2-3 lettere suggerisce fino a 8 candidati (titolo, poi autori; anche gli appunti ◆). Mentre digiti i nodi corrispondenti si **evidenziano** (anello tratteggiato, il resto si attenua); Invio o clic **centra la vista** sulla stella con un alone pulsante e apre la sua scheda. × o Esc pulisce. Trova solo ciò che è nel grafo (documenti con indice semantico).

## 0.9.14 — Recupero metadati: in blocco + per documento con conferma
- **«✦ senza metadati» potenziato**: il recupero in blocco copre tutti i documenti incompleti (titolo/anno/autori mancanti, non più solo quelli senza DOI), recupera i paper arXiv dall'**id nel nome del file** (funziona anche sulle scansioni senza testo) e mostra **barra di avanzamento con Stop**. Precision-first invariato: si applica solo un abbinamento sicuro.
- **«Recupera metadati…» sulla scheda** (tasto destro / ⋯ → Organizza; «Trova…» in Salute libreria): ricerca **estesa** su Crossref, arXiv e OpenAlex, più ogni DOI/arXiv stampato nel PDF e nel nome del file; i **candidati** mostrano le prove riscontrate nel PDF (titolo, autori, anno, DOI) e applichi tu quello giusto — o incolli un DOI/arXiv. I probabili duplicati (DOI già in libreria) sono segnalati.

## 0.9.13 — Costellazione: posizioni degli appunti + nebulose leggibili
- Le posizioni dei nodi **appunto** nel grafo sono persistite tra le sessioni; nomi delle comunità su **targhette** leggibili sopra tutto, con selettore Nebulose+nomi / Solo nebulose / Senza (ricordato).
- Prima release pubblicata con **installer allegato** su GitHub Releases.

## 0.9.12 — Rete di sicurezza + avviso versione + rifiniture QA
- **Backup automatico del database** a ogni cambio di versione dell'app, prima delle migrazioni (in `backups/`, ultime 5 copie).
- **Controlla aggiornamenti** (Sistema → menu): confronto read-only con GitHub, segnalino in header se c'è una versione nuova — nessun download automatico. All'avvio è silenzioso, opt-in (scoperta online attiva) e al più quotidiano.
- README e CHANGELOG nel repository.
- QA sulle superfici recenti: correzioni a lettore/palette/progetti (vedi commit).

## 0.9.x — Interfaccia, guida, palette
- **0.9.11** Palette completa: appunti, pagine wiki, progetti LaTeX e sezioni della guida raggiungibili per nome; Ctrl+K anche dentro il lettore. Guida: sezione «Condividere e stampare» + FAQ.
- **0.9.9 / 0.9.10** Guida riscritta a 7 schede con FAQ; poi trasformata in finestra flottante trascinabile con «in primo piano» opzionale e pulsante dedicato in barra (icona salvagente).
- **0.9.6 → 0.9.8** Menu radiale ridisegnato per ~20 voci: raggio adattivo, etichette a callout lungo i raggi con anti-collisione, descrizione in barra sotto l'anello.

## 0.9.4 / 0.9.5 — Progetti LaTeX
Un piccolo Overleaf locale: progetti come cartelle vere, editor con autosalvataggio, «Cita» dalla libreria, `refs.bib` sincronizzato, compilazione via Tectonic / MiKTeX (texify, senza Perl) / latexmk con anteprima PDF in-app; 5 modelli integrati + «Da .zip…» per i template scaricati (Overleaf/IEEE/ACM/…).

## 0.9.0 → 0.9.3 — Costellazione 2.0
Il grafo semantico diventa uno strumento di scoperta: badge di stato, posizioni persistite (seed PCA), comunità con nebulose ed etichette, «Colora per», stelle fantasma da OpenAlex (citazioni/simili/autore) con aggiunta al volo, appunti nel grafo come rombi; tre iterazioni di fisica del layout.

## 0.8.x — Estrazione, appunti, AI
- **Formule → LaTeX** con math-OCR locale (pix2tex, poi **Pix2Text-MFR 1.5**), beam search, multi-riga, rescue a bisezione, anteprima modificabile; in alternativa vision-LLM (Ollama/LM Studio).
- **Tabelle**: terzo motore **TATR** (struttura dall'immagine, testo esatto dal PDF); **testo formattato** (corsivi/apici/pedici → Markdown); **figure → PNG**; scorciatoie T/X/F/G; finestre trascinabili.
- **Appunti .md**: vault di file veri con `[[wikilink]]`, backlink, ricerca FTS, formule KaTeX, immagini in `assets/`, modalità affiancata, export MD/HTML/LaTeX/PDF; «manda agli Appunti» da lettore e pannelli con citazione `[[@citekey]]`.
- **KaTeX in-app** (0.8.33) per tutta la matematica; MathML solo negli export autonomi.
- Controlli **Memoria AI** (libera GPU / ferma davvero il server), chip AI sempre visibile, «Riprendi lettura», barra strumenti riorganizzata.

## 0.6.0 → 0.8.1 — Le cinque idee del piano
- **Novità** (0.6.0): ricerche salvate rilanciate a ogni avvio, feed persistente con campana.
- **scriptorium-cli**: interrogazione read-only della libreria da terminale.
- **Import LaTeX .zip** (0.7.0): i tuoi paper con bibliografia, «Il mio lavoro», risoluzione DOI.
- **Appunti .md** (0.8.0) e **multi-cite** (0.8.1).

## 0.2.0 → 0.5.x — Fondamenta «Orbita»
Redesign dell'interfaccia (menu radiale, palette Ctrl+K, Costellazione, Lente AI), Wiki della libreria, strumenti di sintesi (Confronta/Rassegna/Tabella risultati/Percorso di lettura), mappa citazioni, pannello di dettaglio, metadati precision-first, connettore browser, import per identificatore/URL.
