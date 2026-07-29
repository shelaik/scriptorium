// Dizionario inglese: chiave = la stringa ITALIANA che compare nel sorgente.
//
// Non esiste un `it.ts`: in italiano `t()` restituisce la chiave stessa. Quindi
// una voce mancante qui significa «resta in italiano», non «si rompe».
//
// Convenzioni:
//  - i segnaposto `{nome}` devono comparire in ENTRAMBE le lingue, con lo stesso
//    nome (l'ordine puo' cambiare: e' il senso dei segnaposto nominati);
//  - le chiavi con `|` portano un contesto di disambiguazione che NON va
//    tradotto e non viene mostrato: `"Nota|del documento": "Note"`;
//  - le forme singolare e plurale sono due chiavi distinte (vedi `tp`).
//
// `node scripts/i18n-check.mjs` elenca le chiavi presenti nel sorgente e non
// qui (da tradurre) e quelle qui e non nel sorgente (orfane, da togliere).




export const EN: Record<string, string> = {
  // ---- correzioni della review: condivisione, stampa, titoli dei modali AI ----
  "Niente da condividere":
    "Nothing to share",
  "Ti condivido: {oggetto}":
    "Sharing with you: {oggetto}",
  "Outlook: PDF allegato a una nuova email":
    "Outlook: PDF attached to a new email",
  "App aperta. Nessun PDF allegabile (riferimenti senza file).":
    "App opened. No PDF to attach (reference-only entries have no file).",
  "PDF copiato":
    "PDF copied",
  "{n} PDF copiati":
    "{n} PDFs copied",
  " ({n} senza file saltati)":
    " ({n} without a file were skipped)",
  "{verbo}: incolla nella conversazione con Ctrl+V{coda}":
    "{verbo}: paste it into the conversation with Ctrl+V{coda}",
  "il PDF non è più al percorso salvato — aprilo dalla libreria e usa «Ritrova il file…»":
    "the PDF is no longer at the saved path — open it from the library and use “Find the file…”",
  "Confronto di {n} paper":
    "Comparison of {n} papers",
  "Rassegna di {n} paper":
    "Review of {n} papers",

  "Lingua delle risposte dell'AI":
    "Language of the AI's answers",
  "Come l'interfaccia":
    "Same as the interface",
  "adesso: italiano":
    "currently: Italian",
  "adesso: inglese":
    "currently: English",
  "Sempre italiano":
    "Always Italian",
  "Sempre inglese":
    "Always English",
  "Vale per riassunti, «Chiedi alla libreria», Lente AI e wiki. È separata perché si può voler leggere il programma in una lingua e farsi riassumere i paper in un'altra. I tag automatici restano sempre in inglese: sono un vocabolario condiviso e mescolarli spaccherebbe in due la tua tassonomia.":
    "Applies to summaries, “Ask your library”, the AI lens and the wiki. It is a separate setting because you may well want to read the program in one language and have your papers summarised in another. Automatic tags always stay in English: they are a shared vocabulary, and mixing languages would split your taxonomy in two.",
  "La lingua dell'interfaccia cambia subito, senza riavviare. La scelta è tua e resta: cambiare la lingua di Windows non cambia quella di Scriptorium.":
    "The interface language changes right away, no restart needed. Your choice sticks: changing the Windows language does not change Scriptorium's.",
  "La tua libreria non viene toccata: titoli, appunti e tag restano come li hai scritti. Cambia solo il testo del programma.":
    "Your library is untouched: titles, notes and tags stay exactly as you wrote them. Only the program's own text changes.",
  "SENZA RACCOLTA":
    "UNFILED",
  "Errore albero: {err}":
    "Error loading the tree: {err}",
  "Errore documenti: {err}":
    "Error loading the documents: {err}",
  "Specchio spento (la cartella resta com'è; i tuoi file veri non sono mai stati toccati)":
    "Mirror off (the folder stays as it is; your real files were never touched)",
  "Scegli la cartella dello specchio (vuota o dedicata)":
    "Choose the mirror folder (an empty or dedicated one)",
  "Specchio attivo: genero l'albero in {cartella}":
    "Mirror on: building the folder tree in {cartella}",
  "1 hardlink":
    "1 hardlink",
  "{n} hardlink":
    "{n} hardlinks",
  "1 copia":
    "1 copy",
  "{n} copie":
    "{n} copies",
  "1 cartella":
    "1 folder",
  "{n} cartelle":
    "{n} folders",
  "1 mancante":
    "1 missing",
  "{n} mancanti":
    "{n} missing",
  "Specchio rigenerato: {dettaglio}":
    "Mirror rebuilt: {dettaglio}",
  "Nessun candidato (prova a togliere «solo senza raccolta»)":
    "No candidates (try clearing “unfiled only”)",
  "Nessun candidato con un vettore semantico":
    "No candidate has a semantic vector yet",
  "1 paper aggiunto a «{raccolta}»":
    "1 paper added to “{raccolta}”",
  "{n} paper aggiunti a «{raccolta}»":
    "{n} papers added to “{raccolta}”",
  "Senza titolo":
    "Untitled",
  "{testa} — {titolo}":
    "{testa} — {titolo}",
  "Raccolta «{nome}» creata":
    "Collection “{nome}” created",
  "Raccolta eliminata (sotto-raccolte risalite, nessun paper perso)":
    "Collection deleted (sub-collections moved up one level, no paper lost)",
  "«{nome}» portata alla radice":
    "“{nome}” moved to the root",
  "«{nome}» annidata":
    "“{nome}” nested",
  "Tolto dalla raccolta":
    "Removed from the collection",
  "Aggiunto anche qui (appartenenza multipla)":
    "Added here too (it can sit in several collections)",
  "Aggiunto alla raccolta":
    "Added to the collection",
  "Spostato":
    "Moved",
  "{autori} et al.":
    "{autori} et al.",
  "ARCHIVIO":
    "ARCHIVE",
  "// RACCOLTE":
    "// COLLECTIONS",
  "{n} PAPER":
    "{n} PAPERS",
  "{n} RACCOLTE":
    "{n} COLLECTIONS",
  "{n} SENZA RACCOLTA":
    "{n} UNFILED",
  "nome della raccolta…":
    "collection name…",
  "CREA":
    "CREATE",
  "ANNULLA":
    "CANCEL",
  "+ NUOVA RACCOLTA":
    "+ NEW COLLECTION",
  "Specchio attivo in {cartella} — clic per spegnerlo":
    "Mirror running in {cartella} — click to switch it off",
  "Proietta le raccolte in una cartella leggibile (hardlink: zero spazio extra, i file veri non si toccano)":
    "Projects your collections into a browsable folder (hardlinks: no extra disk space, the real files are never touched)",
  "SPECCHIO":
    "MIRROR",
  "Ricostruisci ora l'albero su disco":
    "Rebuild the folder tree on disk now",
  "RIGENERA":
    "REBUILD",
  "Apri la cartella dello specchio":
    "Open the mirror folder",
  "APRI":
    "OPEN",
  "Trascina un":
    "Drag a",
  "paper|trascinamento":
    "paper",
  "(dall'elenco a destra) su una raccolta per spostarlo —":
    "(from the list on the right) onto a collection to move it —",
  "= aggiungi anche lì, l'appartenenza è multipla;":
    "= add it there too, a paper can sit in several collections;",
  "sullo sfondo vuoto":
    "onto empty background",
  "= toglilo dalla raccolta · trascina una":
    "= take it out of the collection · drag a",
  "raccolta|trascinamento":
    "collection",
  "su un'altra per annidarla, sullo sfondo per riportarla alla radice":
    "onto another one to nest it, onto the background to bring it back to the root",
  "Raccolte — frecce per spostarti, Invio per aprire nella griglia":
    "Collections — arrow keys to move around, Enter to open in the grid",
  "GERARCHIA":
    "HIERARCHY",
  "{nome} — 1 paper":
    "{nome} — 1 paper",
  "{nome} — {n} paper":
    "{nome} — {n} papers",
  "1 paper · SMART (si popola da sola)":
    "1 paper · SMART (fills itself)",
  "{n} paper · SMART (si popola da sola)":
    "{n} papers · SMART (fills itself)",
  "1 paper":
    "1 paper",
  "{n} paper":
    "{n} papers",
  "diretti {n}":
    "{n} direct",
  "appartenenze nel ramo {n}":
    "{n} memberships in the branch",
  "SMART — si popola da sola":
    "SMART — fills itself",
  "APRI NELLA GRIGLIA":
    "OPEN IN THE GRID",
  "La Costellazione ristretta a questa raccolta (vicinanze ricalcolate al suo interno)":
    "The Constellation narrowed to this collection (proximities recomputed inside it)",
  "COSTELLAZIONE":
    "CONSTELLATION",
  "SALVA":
    "SAVE",
  "RINOMINA":
    "RENAME",
  "nome sotto-raccolta…":
    "sub-collection name…",
  "+ SOTTO-RACCOLTA":
    "+ SUB-COLLECTION",
  "CONFERMI L'ELIMINAZIONE?":
    "CONFIRM DELETION?",
  "NO":
    "NO",
  "ELIMINA":
    "DELETE",
  "Eliminare una raccolta non tocca i paper: le sotto-raccolte risalgono di un livello.":
    "Deleting a collection does not touch the papers: its sub-collections move up one level.",
  "RICERCA «NOVITÀ»":
    "“WHAT'S NEW” SEARCH",
  "ATTIVA ●":
    "ON ●",
  "SPENTA ○":
    "OFF ○",
  "Con la ricerca attiva, a ogni avvio Scriptorium cerca online nuovi paper per questa raccolta (query = il nome; puoi raffinarla fra le ricerche salvate di «Novità»). Le novità accettate dal feed":
    "With the search on, at every start-up Scriptorium looks online for new papers for this collection (the query is its name; you can refine it among the saved “What's new” searches). Feed items you accept",
  "entrano già nella raccolta":
    "go straight into the collection",
  "; con ≥3 paper indicizzati i risultati sono filtrati per somiglianza semantica. Spegnendola, la ricerca (e il suo feed)":
    "; with ≥3 indexed papers the results are filtered by semantic similarity. Switch it off and the search (and its feed)",
  "viene rimossa":
    "is removed",
  "dalle ricerche salvate.":
    "from your saved searches.",
  "✦ SUGGERIMENTI · CALCOLO…":
    "✦ SUGGESTIONS · COMPUTING…",
  "✦ SUGGERIMENTI ({n} sopra soglia)":
    "✦ SUGGESTIONS ({n} above threshold)",
  "✦ SUGGERIMENTI":
    "✦ SUGGESTIONS",
  "Somiglianza col solo NOME della raccolta (utile su raccolte nuove dal titolo parlante)":
    "Similarity against the collection's NAME alone (handy for a brand-new collection with a telling name)",
  "NOME":
    "NAME",
  "Serve almeno un paper nella raccolta":
    "Needs at least one paper in the collection",
  "Somiglianza col solo CONTENUTO (centroide dei paper già dentro) — ignora il nome":
    "Similarity against the CONTENT alone (the centroid of the papers already in it) — the name is ignored",
  "CONTENUTO":
    "CONTENT",
  "Miscela nome+contenuto, col peso qui sotto":
    "Blends name and content, with the weight set below",
  "ENTRAMBI":
    "BOTH",
  "Quanto pesa il CONTENUTO nella miscela (il resto è il nome)":
    "How much the CONTENT weighs in the blend (the rest is the name)",
  "contenuto {a}% · nome {b}%":
    "content {a}% · name {b}%",
  "solo senza raccolta":
    "unfiled only",
  "CALCOLO…":
    "COMPUTING…",
  "CALCOLA I SUGGERIMENTI":
    "COMPUTE THE SUGGESTIONS",
  "confidenza ≥ {soglia}%":
    "confidence ≥ {soglia}%",
  "AGGIUNGO…":
    "ADDING…",
  "AGGIUNGI TUTTI ({n})":
    "ADD ALL ({n})",
  "Aggiungi alla raccolta":
    "Add to the collection",
  "Niente sopra il {soglia}% — abbassa la soglia.":
    "Nothing above {soglia}% — lower the threshold.",
  "Punteggio = somiglianza semantica (bge-m3, in locale) con la sorgente scelta sopra: il":
    "Score = semantic similarity (bge-m3, on your own machine) with the source picked above: the",
  "nome|sorgente dei suggerimenti":
    "name",
  "della raccolta, il suo":
    "of the collection, its",
  "contenuto|sorgente dei suggerimenti":
    "content",
  "(i paper già dentro — più ne aggiungi, meglio va; niente rete né Ollama), o la miscela col peso che preferisci. Nulla viene aggiunto senza il tuo clic.":
    "(the papers already in it — the more you add, the better it gets; no network, no Ollama), or the blend with the weight you prefer. Nothing is added without your click.",
  "I paper non ancora archiviati. Trascinali su una raccolta a sinistra; trascinare qui un paper (da una raccolta) lo toglie da quella raccolta.":
    "The papers not filed yet. Drag them onto a collection on the left; dragging a paper here (from a collection) takes it out of that collection.",
  "PAPER …":
    "PAPERS …",
  "PAPER ({n})":
    "PAPERS ({n})",
  "{documento} — trascinami su una raccolta":
    "{documento} — drag me onto a collection",
  "Carico…":
    "Loading…",
  "Nessun paper qui.":
    "No papers here.",
  "→ radice":
    "→ root",
  "→ sotto-raccolta":
    "→ sub-collection",
  "→ togli dalla raccolta":
    "→ take out of the collection",
  "→ qui":
    "→ here",
  "1 citazione":
    "1 citation",
  "{n} citazioni":
    "{n} citations",
  "✓ già in libreria":
    "✓ already in your library",
  "○ non in libreria — clicca per aggiungerla":
    "○ not in your library — click to add it",
  "Mappa delle citazioni di {titolo}":
    "Citation map for {titolo}",
  "← si fonda su":
    "← builds on",
  "è citato da →":
    "cited by →",
  "Mostro i primi {max} per lato ({rif} riferimenti, {cit} citazioni in tutto — la Lista li ha tutti).":
    "Showing the first {max} per side ({rif} references, {cit} citations in total — the List tab has them all).",
  "Cerca comandi, documenti, filtri…":
    "Search commands, documents, filters…",
  "Palette comandi":
    "Command palette",
  "Cerca comandi":
    "Search commands",
  "Risultati":
    "Results",
  "Recenti":
    "Recent",
  "Nessun comando trovato per «{q}».":
    "No command matches “{q}”.",
  "Le azioni su un documento compaiono qui quando ne hai uno a fuoco (clic su una scheda).":
    "Actions on a document appear here once one has focus (click a card).",
  "Prova anche il":
    "Try a",
  "tasto destro":
    "right-click",
  "su una scheda, o apri la":
    "on a card, or open the",
  "Guida":
    "Guide",
  "dalla barra.":
    "from the toolbar.",
  "↑↓ naviga · Invio esegue · Esc chiude":
    "↑↓ navigate · Enter runs · Esc closes",
  "Tag dominante":
    "Dominant tag",
  "Comunità semantiche":
    "Semantic communities",
  "Anno":
    "Year",
  "Stato lettura":
    "Read status",
  "Nebulose + nomi":
    "Nebulae + names",
  "Solo nebulose":
    "Nebulae only",
  "Senza nebulose":
    "No nebulae",
  "appunto":
    "note",
  "s.d.":
    "n.d.",
  "stella fantasma — clic per la scheda":
    "ghost star — click for its card",
  "Nascondi le stelle fantasma":
    "Hide the ghost stars",
  "clic = scheda · doppio clic = apri · ✦ preferito · ◆ appunto · da vicino: ✓ peer-reviewed, ⑂ GitHub":
    "click = card · double click = open · ✦ favorite · ◆ note · up close: ✓ peer-reviewed, ⑂ GitHub",
  "Cerca nel grafo…":
    "Search the graph…",
  "Trova un paper o un appunto nel grafo: digita qualche lettera del titolo (o di un autore), scegli il candidato e la vista si centra lì":
    "Find a paper or a note in the graph: type a few letters of the title (or of an author), pick a candidate and the view centers on it",
  "Pulisci la ricerca (togli anche l'evidenziazione)":
    "Clear the search (this also removes the highlighting)",
  "Candidati":
    "Candidates",
  "Niente nel grafo (ci sono solo i documenti con indice semantico).":
    "Nothing in the graph (only documents with a semantic index are in here).",
  "Colora le stelle per…":
    "Color the stars by…",
  "Nebulose delle comunità: aloni con i nomi, solo gli aloni, oppure niente":
    "Community nebulae: halos with their names, halos only, or nothing",
  "Densità del grafo (vicini e soglia di somiglianza)":
    "Graph density (neighbors and similarity threshold)",
  "Adatta alla vista":
    "Fit to view",
  "Ingrandisci":
    "Zoom in",
  "Riduci":
    "Zoom out",
  "Ricarica il grafo":
    "Reload the graph",
  "Densità del grafo":
    "Graph density",
  "Quanti vicini più simili collegare a ogni paper. Più alto = rete più fitta e cluster più fusi; più basso = mappa più rada e leggibile.":
    "How many of the most similar neighbors to link to each paper. Higher = a denser web and more merged clusters; lower = a sparser, more readable map.",
  "Legami per nodo":
    "Links per node",
  "Somiglianza minima perché un legame esista (0-100%). Più alta = restano solo i legami forti (mappa più frammentata ma affidabile); più bassa = più connessioni, anche deboli.":
    "Minimum similarity for a link to exist (0-100%). Higher = only the strong links survive (a more fragmented but more trustworthy map); lower = more connections, weak ones included.",
  "Soglia somiglianza":
    "Similarity threshold",
  "Ricalcola":
    "Recompute",
  "Annulla":
    "Cancel",
  "Scheda del paper":
    "Paper card",
  "Chiudi la scheda":
    "Close the card",
  "◆ Appunto (.md)":
    "◆ Note (.md)",
  "✦ preferito":
    "✦ favorite",
  "da leggere":
    "unread",
  "✓ peer-reviewed":
    "✓ peer-reviewed",
  "⑂ GitHub":
    "⑂ GitHub",
  "{autori} e altri":
    "{autori} and others",
  "Mostra la scheda di questo paper":
    "Show this paper's card",
  "{titolo} ({anno})":
    "{titolo} ({anno})",
  "Nessun legame sopra la soglia.":
    "No links above the threshold.",
  "Esplora dintorni (online)":
    "Explore the surroundings (online)",
  "Chi cita e chi è citato da questo paper (OpenAlex) — via DOI, o per titolo se manca":
    "Who cites this paper and who it cites (OpenAlex) — by DOI, or by title when there is no DOI",
  "Citazioni":
    "Citations",
  "Paper simili per argomento (OpenAlex)":
    "Papers on similar topics (OpenAlex)",
  "Simili":
    "Similar",
  "Altri lavori del primo autore":
    "Other work by the first author",
  "Autore":
    "Author",
  "I risultati appaiono come stelle tratteggiate attorno a questa.":
    "The results appear as dashed stars around this one.",
  "Apri l'appunto":
    "Open the note",
  "Apri il paper":
    "Open the paper",
  "o doppio clic sulla stella":
    "or double-click the star",
  "Scheda della scoperta":
    "Discovery card",
  "Chiudi":
    "Close",
  "✦ Stella fantasma — trovata online":
    "✦ Ghost star — found online",
  "Esplora da questa scoperta":
    "Explore on from this discovery",
  "Chi cita e chi è citato da questa scoperta (OpenAlex)":
    "Who cites this discovery and who it cites (OpenAlex)",
  "Altri lavori di {autore}":
    "Other work by {autore}",
  "Autore non noto per questa scoperta":
    "No author known for this discovery",
  "Le nuove stelle si agganciano a questa, in catena — puoi continuare a scavare senza aggiungere nulla. Trascina una scoperta per spostarla: il suo gruppo la segue.":
    "New stars hook onto this one, forming a chain — you can keep digging without adding anything to the library. Drag a discovery to move it: its group follows along.",
  "✓ Già nella libreria":
    "✓ Already in the library",
  "✓ Aggiunto — entrerà nel grafo al prossimo aggiornamento dell'indice":
    "✓ Added — it will join the graph at the next index update",
  "Aggiungi alla libreria":
    "Add to the library",
  "Calcolo la mappa semantica…":
    "Computing the semantic map…",
  "Mappa non disponibile":
    "Map unavailable",
  "Non sono riuscito a caricare il grafo semantico. Controlla che il backend sia attivo e riprova.":
    "The semantic graph could not be loaded. Check that the backend is running, then try again.",
  "Riprova":
    "Try again",
  "La costellazione ha bisogno dell'indice semantico":
    "The constellation needs the semantic index",
  "Per disegnare la mappa servono gli embedding dei documenti: ogni stella è un documento e i legami nascono dalla somiglianza dei contenuti. Genera l'indice per accendere il cielo.":
    "Drawing the map takes the documents' embeddings: every star is a document, and the links come from how similar their contents are. Generate the index to light up the sky.",
  "Avvio…":
    "Starting…",
  "Genera indice":
    "Generate index",
  "Nessun legame sopra la soglia — genera più embedding o riduci la soglia col ricalcolo":
    "No links above the threshold — generate more embeddings, or lower the threshold and recompute",
  "1 documento":
    "1 document",
  "{n} documenti":
    "{n} documents",
  "1 legame":
    "1 link",
  "{n} legami":
    "{n} links",
  "1 connessione":
    "1 connection",
  "{n} connessioni":
    "{n} connections",
  "1 fantasma":
    "1 ghost",
  "{n} fantasmi":
    "{n} ghosts",
  "1 legame per somiglianza":
    "1 similarity link",
  "{n} legami per somiglianza":
    "{n} similarity links",
  "Dettaglio documento":
    "Document details",
  "Tutte le azioni (come il tasto destro)":
    "All actions (same as right-click)",
  "Azioni":
    "Actions",
  "doppio click o Invio per leggere":
    "double-click or Enter to read",
  "Chiudi il pannello (Esc)":
    "Close the panel (Esc)",
  "Riferimento — senza PDF":
    "Reference — no PDF",
  "Letto":
    "Read",
  "Letto al {pct}%":
    "{pct}% read",
  "Tutti i lavori di {autore}":
    "All works by {autore}",
  "Copia la citekey":
    "Copy the citekey",
  "copiata ✓":
    "copied ✓",
  "Apri nel lettore":
    "Open in the reader",
  "Nessun PDF allegato":
    "No PDF attached",
  "Apri":
    "Open",
  "Togli dai preferiti":
    "Remove from favorites",
  "Aggiungi ai preferiti":
    "Add to favorites",
  "Preferito":
    "Favorite",
  "Segna come da leggere":
    "Mark as unread",
  "Segna come letto":
    "Mark as read",
  "Allega PDF…":
    "Attach a PDF…",
  "Tag":
    "Tags",
  "Togli questo tag":
    "Remove this tag",
  "Togli il tag {nome}":
    "Remove the tag {nome}",
  "aggiungi tag…":
    "add a tag…",
  "Aggiungi il tag":
    "Add the tag",
  "Aggiungi tag":
    "Add tag",
  "Riassunto AI":
    "AI summary",
  "Manda il riassunto agli Appunti (con citazione a questo paper)":
    "Send the summary to your Notes (with a citation to this paper)",
  "→ Appunti":
    "→ Notes",
  "genero…":
    "generating…",
  "Genera riassunto":
    "Generate summary",
  "Manda l'abstract agli Appunti (con citazione a questo paper)":
    "Send the abstract to your Notes (with a citation to this paper)",
  "meno":
    "less",
  "tutto":
    "more",
  "Nota del documento":
    "Document note",
  "1 riferimento":
    "1 reference",
  "{n} riferimenti":
    "{n} references",
  "{n} riferimenti ({inLib} in libreria)":
    "{n} references ({inLib} in your library)",
  "{riferimenti} · citato da {citanti} tuoi documenti":
    "{riferimenti} · cited by {citanti} of your documents",
  "Riferimenti e citazioni…":
    "References and citations…",
  "Errore caricamento: {err}":
    "Could not load: {err}",
  "Modifica metadati":
    "Edit metadata",
  "Caricamento…":
    "Loading…",
  "Titolo":
    "Title",
  "Autori (uno per riga)":
    "Authors (one per line)",
  "Nome Cognome":
    "First name Last name",
  "Rivista / Venue":
    "Journal / Venue",
  "La tua nota su questo paper (diversa dagli Appunti .md)":
    "Your own note on this paper (separate from the .md Notes)",
  "Riassunto (AI)":
    "Summary (AI)",
  "Salva":
    "Save",
  "Errore ricerca: {err}":
    "Search error: {err}",
  "identificativo inserito":
    "identifier you entered",
  "Recupera metadati":
    "Fetch metadata",
  "Cerco su Crossref, arXiv e OpenAlex — identificativi stampati nel PDF, nome del file, titolo…":
    "Searching Crossref, arXiv and OpenAlex — identifiers printed in the PDF, the file name, the title…",
  "Titolo rilevato nel PDF:":
    "Title found in the PDF:",
  "File: {nome}":
    "File: {nome}",
  "corrispondenza sicura":
    "certain match",
  "Già in libreria come «{titolo}» — probabilmente è un duplicato: meglio unirli (Cura della libreria → Duplicati).":
    "Already in your library as “{titolo}” — probably a duplicate: better to merge them (Library care → Duplicates).",
  "Applica titolo, autori, anno, rivista (e riferimenti se c'è il DOI) a questo documento":
    "Apply title, authors, year and journal (and the references, when there is a DOI) to this document",
  "Usa questi":
    "Use these",
  "Nessun candidato trovato online (documento non indicizzato, o prima pagina illeggibile). Prova con un identificativo qui sotto, o correggi a mano.":
    "No candidate found online (the document isn't indexed anywhere, or its first page is unreadable). Try an identifier below, or fix the metadata by hand.",
  "Ho il DOI / ID arXiv:":
    "I have the DOI / arXiv ID:",
  "10.1038/nature14539 oppure 2301.12345":
    "10.1038/nature14539 or 2301.12345",
  "Scarico il record completo e lo applico":
    "Downloads the full record and applies it",
  "Applica":
    "Apply",
  "Modifica a mano…":
    "Edit by hand…",
  "Questo PDF è già in libreria su un'altra voce — meglio unire i duplicati":
    "This PDF is already in your library under a different entry — better to merge the duplicates",
  "La voce ha già un PDF":
    "This entry already has a PDF",
  "Nessun link scaricabile per questo candidato — prova il prossimo, o apri la pagina e allega il link qui sotto":
    "No downloadable link for this candidate — try the next one, or open its page and attach the direct link below",
  "Quel PDF è già in libreria (su un'altra voce)":
    "That PDF is already in your library (under a different entry)",
  "Il link non è un PDF diretto (deve scaricare un .pdf)":
    "That link is not a direct PDF (it has to download a .pdf)",
  "Non allegato: {esito}":
    "Not attached: {esito}",
  "Trova PDF":
    "Find PDF",
  "Trova PDF — candidati":
    "Find PDF — candidates",
  "Cerco su arXiv, OpenAlex, Semantic Scholar e Crossref — per identificativo e per titolo…":
    "Searching arXiv, OpenAlex, Semantic Scholar and Crossref — by identifier and by title…",
  "Voce:":
    "Entry:",
  "Scarica il PDF da questa fonte e allegalo a questa voce (senza duplicati)":
    "Download the PDF from this source and attach it to this entry (never creating a duplicate)",
  "scarico…":
    "downloading…",
  "Scarica e allega":
    "Download and attach",
  "Apri la pagina del paper nel browser per controllare":
    "Open the paper's page in the browser to check it before downloading",
  "Apri pagina":
    "Open page",
  "Nessun candidato trovato online. Se conosci la pagina del paper, incolla qui sotto il link diretto al PDF.":
    "No candidates found online. If you know the paper's page, paste the direct link to the PDF below.",
  "Link diretto al PDF:":
    "Direct link to the PDF:",
  "https://…/file.pdf (vanno bene anche le pagine GitHub /blob/)":
    "https://…/file.pdf (GitHub /blob/ pages work too)",
  "Allega":
    "Attach",
  "IMPORT":
    "IMPORT",
  "file · trascina":
    "files · drag & drop",
  "CARTELLA":
    "FOLDER",
  "sorvegliata":
    "watched",
  "BROWSER":
    "BROWSER",
  "connettore":
    "connector",
  "BIBLIOTECHE":
    "REF MANAGERS",
  "Zotero · RIS · CSL":
    "Zotero · RIS · CSL",
  "LATEX":
    "LATEX",
  "progetti · compile":
    "projects · compile",
  "SCOPERTA":
    "DISCOVERY",
  "novità · online":
    "what's new · online",
  "ESTRAZIONE":
    "EXTRACTION",
  "pdfium":
    "pdfium",
  "METADATI":
    "METADATA",
  "Crossref · OpenAlex":
    "Crossref · OpenAlex",
  "MINIATURE":
    "THUMBNAILS",
  "copertine":
    "covers",
  "OCR":
    "OCR",
  "scansioni":
    "scans",
  "FORMULE":
    "FORMULAS",
  "math-OCR":
    "math-OCR",
  "TABELLE":
    "TABLES",
  "TATR":
    "TATR",
  "DATABASE":
    "DATABASE",
  "SQLite · FTS5 · vec":
    "SQLite · FTS5 · vec",
  "papers · note · progetti":
    "papers · notes · projects",
  "BACKUP":
    "BACKUP",
  "copie · ripristino":
    "copies · restore",
  "raccolte su disco":
    "collections on disk",
  "TERMINALE":
    "TERMINAL",
  "PowerShell":
    "PowerShell",
  "EMBEDDING":
    "EMBEDDING",
  "bge-m3 locale":
    "local bge-m3",
  "INDICE RAG":
    "RAG INDEX",
  "passaggi per «Chiedi»":
    "passages for “Ask”",
  "CHIEDI":
    "ASK",
  "domande alla libreria":
    "library questions",
  "WIKI":
    "WIKI",
  "sintesi per concetto":
    "one page per concept",
  "RIASSUNTI":
    "SUMMARIES",
  "AI locale":
    "local AI",
  "DOI RIF.":
    "REF DOIs",
  "backfill riferimenti":
    "reference backfill",
  "Importa PDF scelti a mano o trascinati: lettura, hash anti-doppione, estrazione testo, copertina, commit nel database.":
    "Imports the PDFs you pick or drag in: read, anti-duplicate hash, text extraction, cover, commit to the database.",
  "Sorveglia la cartella scelta nelle Impostazioni: ogni PDF che vi appare viene importato da solo (senza mai ripescare ciò che hai cestinato).":
    "Watches the folder you chose in Settings: every PDF that shows up there is imported on its own (and what you sent to the trash is never dragged back in).",
  "Il connettore browser: il bookmarklet manda l'URL del PDF aperto e Scriptorium lo scarica e importa con lo stesso motore (protetto anti-SSRF).":
    "The browser connector: the bookmarklet sends the URL of the PDF you have open and Scriptorium downloads and imports it with the same engine (SSRF-protected).",
  "Import da gestori bibliografici (Zotero, Mendeley, EndNote, JabRef…): legge .bib/.ris/CSL-JSON, aggancia i PDF, converte le keyword in tag, deduplica per DOI e contenuto.":
    "Import from reference managers (Zotero, Mendeley, EndNote, JabRef…): reads .bib/.ris/CSL-JSON, attaches the PDFs, turns keywords into tags, deduplicates by DOI and by content.",
  "I progetti LaTeX: import degli .zip (PDF + bibliografia nel grafo) e compilazione con Tectonic/latexmk.":
    "LaTeX projects: .zip import (PDFs + bibliography into the graph) and compilation with Tectonic/latexmk.",
  "Il lato online: ricerche su arXiv/OpenAlex/ADS…, lo sweep «Novità» delle ricerche salvate — comprese quelle agganciate alle raccolte dell'Archivio (filtro semantico; le novità accettate entrano da sole nella raccolta; il toggle le crea e le rimuove) — e l'aggiunta di paper trovati.":
    "The online side: searches on arXiv/OpenAlex/ADS…, the “What's new” sweep over your saved searches — including the ones attached to Archive collections (semantic filter; the new papers you accept join the collection on their own; the toggle creates them and removes them) — and adding the papers you find.",
  "Il motore pdfium: estrae testo e rende le pagine. Serializzato: un documento alla volta, per stabilità.":
    "The pdfium engine: extracts text and renders pages. Serialized: one document at a time, for stability.",
  "Identità dei documenti: DOI dal testo, titolo/autori/anno da Crossref e OpenAlex, con i filtri di precisione (mai etichettare col paper sbagliato).":
    "Document identity: DOI from the text, title/authors/year from Crossref and OpenAlex, with the precision filters (never label a document with the wrong paper).",
  "Le copertine della griglia, rese da pdfium e salvate in cache.":
    "The covers you see in the grid, rendered by pdfium and cached.",
  "Riconoscimento testo per i PDF scansionati (immagini → testo cercabile).":
    "Text recognition for scanned PDFs (images → searchable text).",
  "Math-OCR locale (pix2tex via ONNX): un ritaglio di formula diventa LaTeX. I modelli si scaricano al primo uso.":
    "Local math-OCR (pix2tex via ONNX): a crop of a formula becomes LaTeX. The models are downloaded on first use.",
  "Riconoscimento struttura tabelle (TATR): un ritaglio diventa righe e colonne vere.":
    "Table structure recognition (TATR): a crop becomes real rows and columns.",
  "Il cuore: SQLite con ricerca full-text (FTS5) e vettoriale (sqlite-vec). Ogni luce qui è una modifica alla libreria.":
    "The heart: SQLite with full-text (FTS5) and vector (sqlite-vec) search. Every light here is a change to the library.",
  "L'organizzazione della libreria: raccolte e sotto-raccolte (vista Archivio) coi suggerimenti semantici per riempirle — sorgente a scelta: nome, contenuto o miscela pesata; il nome si àncora ai tuoi paper anche a motori spenti — e i file veri su disco (papers/, note .md, progetti LaTeX, in %APPDATA%).":
    "How the library is organized: collections and sub-collections (Archive view) with semantic suggestions to fill them — source of your choice: name, content or a weighted blend; the name anchors itself to your own papers even with the engines off — and the real files on disk (papers/, .md notes, LaTeX projects, under %APPDATA%).",
  "Lo Specchio su disco: proietta le raccolte in una cartella leggibile (Raccolta\\Sottoraccolta\\Autore Anno — Titolo.pdf) con hardlink, sincronizzata da sola a ogni cambio. La libreria vera non viene mai toccata; si attiva dall'Archivio.":
    "The disk Mirror: projects your collections into a readable folder (Collection\\Subcollection\\Author Year — Title.pdf) using hardlinks, kept in sync on its own at every change. The real library is never touched; you turn it on from the Archive.",
  "Copie di sicurezza complete e ripristino (validato, atomico, con copia pre-ripristino).":
    "Full safety copies and restore (validated, atomic, with a copy taken before restoring).",
  "La PowerShell integrata, aperta nella cartella dei PDF.":
    "The built-in PowerShell, opened in the PDF folder.",
  "Vettori semantici bge-m3 (locale, ONNX): alimentano Correlati, ricerca per significato e Costellazione.":
    "bge-m3 semantic vectors (local, ONNX): they feed Related, search by meaning and the Constellation.",
  "L'indice a passaggi per «Chiedi alla libreria»: spezza i documenti e li vettorizza per il recupero citato.":
    "The passage index behind “Ask the library”: it splits your documents and vectorizes them so answers can cite what they used.",
  "Domande alla tua libreria: recupero dei passaggi pertinenti + risposta del modello locale con citazioni [n].":
    "Questions to your own library: the relevant passages are retrieved, then the local model answers with [n] citations.",
  "Genera pagine di sintesi per concetto dai tuoi paper, con fonti.":
    "Generates one synthesis page per concept out of your own papers, with sources.",
  "Riassunti e confronti AI dei singoli documenti (modello locale via Ollama/LM Studio).":
    "AI summaries and comparisons of individual documents (local model via Ollama/LM Studio).",
  "Backfill dei DOI dei riferimenti citati: ricerca su Crossref con il filtro di precisione (mai un DOI sbagliato).":
    "Backfill of the DOIs of cited references: a Crossref search behind the precision filter (never a wrong DOI).",
  "errore":
    "error",
  "online disattivato":
    "online turned off",
  "AI disattivata":
    "AI turned off",
  "spento (si attiva dall'Archivio)":
    "off (turn it on from the Archive)",
  "nessuna cartella":
    "no folder set",
  "connettore spento":
    "connector off",
  "modelli da scaricare":
    "models not downloaded yet",
  "<1s":
    "<1s",
  "{s}s":
    "{s}s",
  "{m}m {s}s":
    "{m}m {s}s",
  "{h}h {m}m":
    "{h}h {m}m",
  "{testo} — {dettaglio}":
    "{testo} — {dettaglio}",
  "{doc} doc · {cest} cestino · {mb} MB":
    "{doc} docs · {cest} trashed · {mb} MB",
  "{fatti}/{totale} vettorizzati":
    "{fatti}/{totale} vectorized",
  "{doc} doc · {pass} passaggi":
    "{doc} docs · {pass} passages",
  "indice da costruire":
    "index not built yet",
  "{racc} raccolte · {app} appunti · {prog} progetti":
    "{racc} collections · {app} notes · {prog} projects",
  "nessun backup":
    "no backup yet",
  "ultimo: oggi":
    "last: today",
  "ultimo: {g} g fa":
    "last: {g} d ago",
  "Salva il registro della Plancia":
    "Save the Bridge log",
  "Testo":
    "Text",
  "errore: {err}":
    "error: {err}",
  "Libreria aggiornata":
    "Library updated",
  "Plancia — Scriptorium":
    "Bridge — Scriptorium",
  "PLANCIA":
    "BRIDGE",
  "SESSIONE {durata} · {ev} EV/MIN":
    "SESSION {durata} · {ev} EV/MIN",
  "RETE ONLINE":
    "NETWORK ONLINE",
  "RETE OFF":
    "NETWORK OFF",
  "AI {modello}":
    "AI {modello}",
  "AI OFF":
    "AI OFF",
  "CONNETTORE":
    "CONNECTOR",
  "Specchio su disco spento (si attiva dall'Archivio)":
    "Disk Mirror is off (you turn it on from the Archive)",
  "Il registro viene scritto anche su file (Impostazioni → Manutenzione)":
    "The log is also being written to a file (Settings → Maintenance)",
  "LOG SU FILE ●":
    "LOG TO FILE ●",
  "SISTEMA IN QUIETE":
    "SYSTEM IDLE",
  "Salva il registro della sessione in un file di testo":
    "Save this session's log to a text file",
  "SALVA REGISTRO…":
    "SAVE LOG…",
  "{nodo} — ERRORE":
    "{nodo} — ERROR",
  "{testo} · {dettaglio}":
    "{testo} · {dettaglio}",
  "SORGENTI":
    "SOURCES",
  "ELABORAZIONE":
    "PROCESSING",
  "NUCLEO":
    "CORE",
  "INTELLIGENZA":
    "INTELLIGENCE",
  "OFFLINE":
    "OFFLINE",
  "ONLINE":
    "ONLINE",
  "in lavorazione":
    "working",
  "con errori":
    "with errors",
  "AVARIA":
    "FAULT",
  "OFFLINE — {motivo}":
    "OFFLINE — {motivo}",
  "AVARIA · {ora}":
    "FAULT · {ora}",
  "PRESA VISIONE":
    "ACKNOWLEDGE",
  "IN LAVORAZIONE · da {durata}":
    "WORKING · for {durata}",
  "IN LAVORAZIONE":
    "WORKING",
  "{testo} — {fatti}/{totale}":
    "{testo} — {fatti}/{totale}",
  "IN QUIETE":
    "IDLE",
  "ultimo esito · {ora}":
    "last result · {ora}",
  "({durata})":
    "({durata})",
  "avvii":
    "starts",
  "ok":
    "ok",
  "errori":
    "errors",
  "ultima durata":
    "last duration",
  "STORICO DEL NODO (sessione)":
    "NODE HISTORY (session)",
  "Nessun evento in questa sessione.":
    "No events in this session.",
  "REGISTRO ATTIVITÀ — {nodo}":
    "ACTIVITY LOG — {nodo}",
  "REGISTRO ATTIVITÀ":
    "ACTIVITY LOG",
  "TUTTI":
    "ALL",
  "ERRORI":
    "ERRORS",
  "Nessun errore registrato. Ottimo segno.":
    "No errors recorded. Good sign.",
  "Nessuna attività registrata da quando l'app è aperta. Lo schema si accende quando qualcosa lavora davvero.":
    "No activity recorded since the app was opened. The diagram lights up when something is really working.",
  "1 ricerca attiva":
    "1 active search",
  "{n} ricerche attive":
    "{n} active searches",
  "1 voce solo-riferimento":
    "1 reference-only entry",
  "{n} voci solo-riferimento":
    "{n} reference-only entries",
  "salvato 1 evento":
    "1 event saved",
  "salvati {n} eventi":
    "{n} events saved",
  "1 ERRORE":
    "1 ERROR",
  "{n} ERRORI":
    "{n} ERRORS",
  "1 PROCESSO ATTIVO":
    "1 ACTIVE PROCESS",
  "{n} PROCESSI ATTIVI":
    "{n} ACTIVE PROCESSES",
  "Indietro":
    "Back",
  "Aggiunto a «{titolo}» ✓":
    "Added to “{titolo}” ✓",
  "Errore: non aggiunto all'appunto ({err})":
    "Error: not added to the note ({err})",
  "Creato l'appunto «{nome}» ✓":
    "Created the note “{nome}” ✓",
  "Errore: appunto non creato ({err})":
    "Error: note not created ({err})",
  "Manda agli Appunti":
    "Send to Notes",
  "aperta":
    "open",
  "Nessun appunto ancora — creane uno qui sotto.":
    "No notes yet — create one below.",
  "＋ Nuovo appunto":
    "＋ New note",
  "＋ Nuovo appunto — «{titolo}»":
    "＋ New note — “{titolo}”",
  "Documento PDF":
    "PDF document",
  "Preparazione condivisione…":
    "Preparing to share…",
  "Errore condivisione: {err}":
    "Sharing failed: {err}",
  "Questo elemento non ha un file da mostrare":
    "This item has no file to show",
  "Condividi via WhatsApp, Teams, Gmail o Outlook":
    "Share via WhatsApp, Teams, Gmail or Outlook",
  "Condividi":
    "Share",
  "Apri cartella del file":
    "Open file location",
  "Errore avvio terminale: {err}":
    "Could not start the terminal: {err}",
  "Apri il terminale in questa cartella":
    "Open the terminal in this folder",
  "[sessione terminata — premi Riavvia per ripartire]":
    "[session ended — press Restart to start again]",
  "Apri il terminale in un'altra cartella":
    "Open the terminal in a different folder",
  "Cambia cartella…":
    "Change folder…",
  "Termina e riavvia la sessione nella stessa cartella":
    "End the session and start a new one in the same folder",
  "Riavvia":
    "Restart",
  "Chiudi il terminale":
    "Close the terminal",
  ".gitignore scritto nella cartella del progetto":
    ".gitignore written in the project folder",
  "Articolo semplice":
    "Simple article",
  "Paper (due colonne)":
    "Paper (two columns)",
  "Relazione / tesi":
    "Report / thesis",
  "Presentazione (beamer)":
    "Presentation (beamer)",
  "Minimale":
    "Minimal",
  "Progetto o template LaTeX (.zip)":
    "LaTeX project or template (.zip)",
  "refs.bib aggiornato: 1 voce dalla libreria":
    "refs.bib updated: 1 entry from the library",
  "refs.bib aggiornato: {n} voci dalla libreria":
    "refs.bib updated: {n} entries from the library",
  "Nuovo progetto…":
    "New project…",
  "Crea":
    "Create",
  "Modello di partenza per «Crea»":
    "Starting template for “Create”",
  "Crea un progetto da uno .zip: il TUO progetto scaricato da Overleaf (Menu → Download → Source) oppure un template (IEEE, ACM…). Estrae tutto: .tex, immagini, classi e sottocartelle.":
    "Create a project from a .zip: YOUR own project downloaded from Overleaf (Menu → Download → Source), or a template (IEEE, ACM…). Everything is extracted: .tex files, images, class files and subfolders.",
  "Da .zip (anche Overleaf)…":
    "From .zip (Overleaf too)…",
  "Nessun progetto. Creane uno: nasce con un `main.tex` di partenza e un `refs.bib` con le citazioni della tua libreria.":
    "No projects yet. Create one: it starts with a `main.tex` to build on and a `refs.bib` holding the citations from your library.",
  "Scarica un template ufficiale come .zip, poi «Da .zip…»":
    "Download an official template as a .zip, then use “From .zip…”",
  "Modelli online:":
    "Templates online:",
  "File|intestazione dell'elenco dei file di un progetto":
    "Files",
  "Apri cartella":
    "Open folder",
  "Scrive un .gitignore adatto a LaTeX — utile se sincronizzi questa cartella con Git (per esempio col ponte Git di Overleaf)":
    "Writes a .gitignore suited to LaTeX — handy if you sync this folder with Git, through Overleaf's Git bridge for instance",
  "salvataggio…":
    "saving…",
  "salvato":
    "saved",
  "Cita":
    "Cite",
  "Cerca nella libreria…":
    "Search the library…",
  "(senza titolo)":
    "(untitled)",
  "senza citekey":
    "no citekey",
  "Nessun risultato.":
    "No results.",
  "Sovrascrivo `refs.bib` con tutta la libreria: quello che c'è ora, comprese le voci scritte a mano, va perso.":
    "Overwriting `refs.bib` with the whole library: what is in it now, hand-written entries included, is lost.",
  "Sovrascrivi":
    "Overwrite",
  "Riscrive refs.bib con tutta la libreria (chiede conferma)":
    "Rewrites refs.bib from the whole library (asks first)",
  "Sincronizza bibliografia":
    "Sync bibliography",
  "Compilo…":
    "Compiling…",
  "Compila":
    "Compile",
  "Compilazione fallita ({strumento}).":
    "Compilation failed ({strumento}).",
  "Compilazione fallita.":
    "Compilation failed.",
  "Nascondi log":
    "Hide log",
  "Mostra log":
    "Show log",
  "PDF prodotto ({strumento}), ma con avvisi o errori nel log.":
    "PDF produced ({strumento}), but the log has warnings or errors.",
  "Compilato con {strumento}.":
    "Compiled with {strumento}.",
  "Scegli un file dal pannello a sinistra.":
    "Pick a file from the panel on the left.",
  "Progetti LaTeX":
    "LaTeX projects",
  "Un piccolo Overleaf locale: i progetti sono cartelle vere in `projects/` dentro i dati dell'app — file `.tex` e `.bib` tuoi, per sempre.":
    "A small local Overleaf: projects are real folders in `projects/` inside the app data — your own `.tex` and `.bib` files, forever.",
  "Per compilare serve un compilatore LaTeX di sistema: va bene **MiKTeX** o TeX Live già installati, oppure **Tectonic** (un solo eseguibile, scarica i pacchetti da solo):":
    "Compiling needs a LaTeX toolchain on the system: **MiKTeX** or TeX Live already installed will do, or **Tectonic** (a single executable that fetches the packages by itself):",
  "Parti da un modello integrato («Crea») o scarica un template ufficiale (Overleaf, IEEE, ACM…) come .zip e usa «Da .zip…».":
    "Start from a built-in template (“Create”), or download an official one (Overleaf, IEEE, ACM…) as a .zip and use “From .zip…”.",
  "Senza compilatore, editor + citazioni + bibliografia funzionano comunque.":
    "Without a compiler, the editor, the citations and the bibliography still work.",
  "Anteprima · 1 pagina (rendering…)":
    "Preview · 1 page (rendering…)",
  "Anteprima · {n} pagine (rendering…)":
    "Preview · {n} pages (rendering…)",
  "Anteprima · 1 pagina":
    "Preview · 1 page",
  "Anteprima · {n} pagine":
    "Preview · {n} pages",
  "Compilazione in corso…":
    "Compiling…",
  "L'anteprima del PDF compilato apparirà qui dopo «Compila».":
    "The compiled PDF will show up here after “Compile”.",
  "(nota)":
    "(note)",
  "(selezione)":
    "(selection)",
  "1 evidenziazione":
    "1 highlight",
  "<kbd>F</kbd>, da ⋯ Altro o dal radiale: trascina attorno a un'equazione. Motore «Locale» (math-OCR integrato, il 1º uso scarica ~180 MB) o «Ollama» (modello di visione). «Più righe» = blocco gathered. Il LaTeX riconosciuto è <strong>modificabile</strong> con <strong>anteprima resa</strong> che si aggiorna mentre correggi; il pulsante <strong>\\mathrm{}</strong> avvolge (o toglie) tutta la formula in tondo. Esporta come LaTeX o Markdown ($$…$$): Copia, Salva… o → Appunti":
    "<kbd>F</kbd>, from ⋯ More or from the radial menu: drag around an equation. Engine «Local» (built-in math-OCR, the 1st run downloads ~180 MB) or «Ollama» (vision model). «Multi-line» = a gathered block. The recognized LaTeX is <strong>editable</strong>, with a <strong>rendered preview</strong> that updates while you fix it; the <strong>\\mathrm{}</strong> button wraps (or unwraps) the whole formula in upright type. Export as LaTeX or Markdown ($$…$$): Copy, Save… or → Notes",
  "<kbd>G</kbd>, da ⋯ Altro o dal radiale: trascina attorno a una figura per ritagliarla come immagine PNG. «Salva PNG…» su file, oppure «→ Appunti» per incorporarla in un appunto":
    "<kbd>G</kbd>, from ⋯ More or from the radial menu: drag around a figure to crop it as a PNG image. «Save PNG…» to a file, or «→ Notes» to embed it in a note",
  "<kbd>T</kbd> / <kbd>X</kbd>. Motore «Nativa» (dal testo del PDF) o «Ollama» (modello di visione — utile per tabelle-immagine e pagine scansionate). Esporta scegliendo il formato: tabella MD/LaTeX/CSV (+ Excel), testo Testo/LaTeX/MD — con Copia, Salva… o → Appunti":
    "<kbd>T</kbd> / <kbd>X</kbd>. Engine «Native» (from the PDF's own text) or «Ollama» (vision model — handy for tables that are images and for scanned pages). Export in the format you pick: table MD/LaTeX/CSV (+ Excel), text Text/LaTeX/MD — with Copy, Save… or → Notes",
  "? Aiuto":
    "? Help",
  "AI non disponibile: {err}":
    "AI not available: {err}",
  "Abilita l'AI locale nelle Impostazioni per usare Ollama":
    "Enable the local AI in Settings to use Ollama",
  "Adatta alla larghezza":
    "Fit to width",
  "Adatta alla larghezza (W)":
    "Fit to width (W)",
  "Adatta alla pagina":
    "Fit to page",
  "Adatta alla pagina (H)":
    "Fit to page (H)",
  "Adatta larghezza":
    "Fit width",
  "Adatta pagina":
    "Fit page",
  "Aggiungi la risposta alla Nota del documento":
    "Add the answer to the document note",
  "Aggiungi nota":
    "Add note",
  "Aggiungi una nota":
    "Add a note",
  "Aggiungi una nota a un punto della pagina (N)":
    "Add a note at one spot on the page (N)",
  "Aggiungi una nota…":
    "Add a note…",
  "Aggiunto alla Nota del documento":
    "Added to the document note",
  "Altri (no visione)":
    "Others (no vision)",
  "Altri strumenti":
    "More tools",
  "Altri strumenti: rotazione, estrazione, stampa, condivisione…":
    "More tools: rotation, extraction, printing, sharing…",
  "Altri {n} file sono nella stessa cartella":
    "{n} more files are in the same folder",
  "Annotazioni":
    "Annotations",
  "Annotazioni ({n})":
    "Annotations ({n})",
  "Annotazioni copiate in Markdown ✓":
    "Annotations copied as Markdown ✓",
  "Annotazioni esportate ✓":
    "Annotations exported ✓",
  "Anteprima come griglia":
    "Preview as a grid",
  "Anteprima del formato scelto":
    "Preview of the chosen format",
  "Anteprima resa":
    "Rendered preview",
  "Anteprima resa (blocco $$…$$)":
    "Rendered preview ($$…$$ block)",
  "Anteprima vuota.":
    "Preview is empty.",
  "Apri la posizione del PDF in Esplora risorse":
    "Open the PDF's location in File Explorer",
  "Arancio":
    "Orange",
  "Avvolgi tutta la formula in \\mathrm{} (la rende tutta in tondo/dritto)":
    "Wrap the whole formula in \\mathrm{} (renders it all upright)",
  "Barra":
    "Strike through",
  "Barrati":
    "Struck through",
  "Blu":
    "Blue",
  "Cerca":
    "Find",
  "Cerca nel documento":
    "Find in the document",
  "Cerca nel documento (Ctrl+F)":
    "Find in the document (Ctrl+F)",
  "Cerca nel documento…":
    "Find in the document…",
  "Chiedi":
    "Ask",
  "Chiudi (Esc)":
    "Close (Esc)",
  "Chiudi / annulla":
    "Close / cancel",
  "Chiudi il lettore (Esc)":
    "Close the reader (Esc)",
  "Chiudi il lettore e apri gli Appunti (.md)":
    "Close the reader and open the Notes (.md)",
  "Chiudi lettore":
    "Close reader",
  "Chiudi pannello":
    "Close the panel",
  "Collegato — attenzione: il file scelto non è identico all'originale.":
    "Linked — careful: the file you picked is not identical to the original.",
  "Copia":
    "Copy",
  "Copia MD":
    "Copy MD",
  "Copia come LaTeX":
    "Copy as LaTeX",
  "Copia il testo nel formato scelto":
    "Copy the text in the chosen format",
  "Copia il testo selezionato: “{testo}”":
    "Copy the selected text: “{testo}”",
  "Copia la formula nel formato scelto":
    "Copy the formula in the chosen format",
  "Copia la selezione come LaTeX (escape dei caratteri speciali)":
    "Copy the selection as LaTeX (special characters escaped)",
  "Copia la tabella nel formato scelto":
    "Copy the table in the chosen format",
  "Copia tutte le annotazioni come Markdown":
    "Copy every annotation as Markdown",
  "Copiato ✓":
    "Copied ✓",
  "Correggi qui il LaTeX: l'anteprima si aggiorna mentre scrivi":
    "Fix the LaTeX here: the preview updates while you type",
  "Dov'è finito questo PDF?":
    "Where did this PDF go?",
  "Due pagine":
    "Two pages",
  "Elimina":
    "Delete",
  "Eliminare?":
    "Delete it?",
  "Eliminazione non riuscita: {err}":
    "Could not delete it: {err}",
  "Errore eliminazione: {err}":
    "Deletion error: {err}",
  "Errore estrazione testo: {err}":
    "Text extraction failed: {err}",
  "Errore export tabella: {err}":
    "Table export failed: {err}",
  "Errore export: {err}":
    "Export failed: {err}",
  "Errore nota: {err}":
    "Note error: {err}",
  "Errore riconoscimento formula: {err}":
    "Formula recognition failed: {err}",
  "Errore salvataggio PNG: {err}":
    "Could not save the PNG: {err}",
  "Errore salvataggio: {err}":
    "Could not save: {err}",
  "Esporta MD":
    "Export MD",
  "Esporta come file Markdown":
    "Export as a Markdown file",
  "Esporta come foglio Excel (.xlsx)":
    "Export as an Excel sheet (.xlsx)",
  "Estraggo il testo…":
    "Extracting the text…",
  "Estraggo la tabella con il modello di visione…":
    "Extracting the table with the vision model…",
  "Estraggo la tabella…":
    "Extracting the table…",
  "Estrai con un modello di visione locale (Ollama / LM Studio) — utile su pagine scansionate":
    "Extract with a local vision model (Ollama / LM Studio) — handy on scanned pages",
  "Estrai figura":
    "Extract figure",
  "Estrai figura: trascina un rettangolo attorno alla figura da ritagliare come PNG":
    "Extract figure: drag a rectangle around the figure to crop as a PNG",
  "Estrai tabella":
    "Extract table",
  "Estrai tabella / testo":
    "Extract table / text",
  "Estrai tabella / testo / formula / figura: attiva la selezione, poi trascina un riquadro sulla pagina (premi di nuovo per annullare)":
    "Extract table / text / formula / figure: arm the selection, then drag a box over the page (press again to cancel)",
  "Estrai testo":
    "Extract text",
  "Estrai testo: trascina un rettangolo attorno al testo (scorciatoia: X)":
    "Extract text: drag a rectangle around the text (shortcut: X)",
  "Estrai testo: trascina un rettangolo attorno al testo da copiare":
    "Extract text: drag a rectangle around the text you want",
  "Estrai una tabella: trascina un rettangolo attorno alla tabella (scorciatoia: T)":
    "Extract a table: drag a rectangle around the table (shortcut: T)",
  "Estrazione nativa dal testo del PDF":
    "Native extraction from the PDF's own text",
  "Euristica veloce sul testo del PDF (tabelle semplici)":
    "Fast heuristic over the PDF's own text (simple tables)",
  "Evidenzia":
    "Highlight",
  "Evidenziazioni":
    "Highlights",
  "Fai una domanda sul testo selezionato…":
    "Ask a question about the selected text…",
  "Fai una domanda sulla selezione":
    "Ask a question about the selection",
  "Figura ritagliata":
    "Cropped figure",
  "Figura salvata ✓":
    "Figure saved ✓",
  "Figura → PNG":
    "Figure → PNG",
  "Filtra colore":
    "Filter color",
  "Filtra per colore":
    "Filter by color",
  "Filtra per tipo":
    "Filter by kind",
  "Finestre di estrazione":
    "Extraction windows",
  "Formato":
    "Format",
  "Formato di esportazione (Copia / Salva / → Appunti)":
    "Export format (Copy / Save / → Notes)",
  "Formattazione del PDF rilevata (corsivo, grassetto, apici/pedici): conservata negli export MD e LaTeX, rimossa in «Testo».":
    "Formatting detected in the PDF (italics, bold, super/subscripts): kept in the MD and LaTeX exports, dropped under «Text».",
  "Formula selezionata":
    "Selected formula",
  "Formula → LaTeX":
    "Formula → LaTeX",
  "Formula → LaTeX: trascina un rettangolo attorno a una formula":
    "Formula → LaTeX: drag a rectangle around a formula",
  "Giallo":
    "Yellow",
  "Griglia":
    "Grid",
  "Hai spostato un'intera cartella: posso ricollegarli tutti allo stesso modo, verificando l'impronta di ciascun file — chi non corrisponde viene lasciato stare.":
    "You moved a whole folder: I can relink them all the same way, checking each file's fingerprint — anything that doesn't match is left alone.",
  "Ho capito":
    "Got it",
  "Il PDF non è più dov'era":
    "The PDF is no longer where it was",
  "Il motore «Modello» non supporta la pagina ruotata: riporta la rotazione a 0°, oppure usa «Nativa» o «Ollama».":
    "The «Model» engine does not support a rotated page: set the rotation back to 0°, or use «Native» or «Ollama».",
  "Immagine del testo non disponibile":
    "The image of the text is not available",
  "Immagine della tabella non disponibile":
    "The image of the table is not available",
  "Immagine o regione della tabella non disponibile":
    "The image or the region of the table is not available",
  "Impossibile aprire il PDF: {err}":
    "Cannot open the PDF: {err}",
  "Impossibile copiare":
    "Cannot copy",
  "Impossibile rendere la formula.":
    "Cannot render the formula.",
  "Impossibile ritagliare la figura":
    "Cannot crop the figure",
  "Impossibile ritagliare la formula":
    "Cannot crop the formula",
  "In attesa della risposta":
    "Waiting for the answer",
  "Incorpora la figura in un appunto come immagine Markdown, con citazione al paper":
    "Embed the figure in a note as a Markdown image, with a citation to the paper",
  "Indice":
    "Contents",
  "Indice del documento":
    "Table of contents of the document",
  "Indice delle annotazioni di questo documento (A)":
    "Index of this document's annotations (A)",
  "Ingrandisci (+, oppure Ctrl + rotella)":
    "Zoom in (+, or Ctrl + wheel)",
  "Ingrandisci / riduci":
    "Zoom in / out",
  "Inverti i colori (I)":
    "Invert the colors (I)",
  "Invia":
    "Send",
  "L'hai spostato o rinominato fuori da Scriptorium. Indicami dov'è ora: appunti, evidenziazioni, tag e nota restano tutti al loro posto.":
    "You moved or renamed it outside Scriptorium. Show me where it is now: notes, highlights, tags and the document note all stay exactly where they are.",
  "La barra si nasconde; muovi il mouse per mostrarla":
    "The toolbar hides; move the mouse to bring it back",
  "LaTeX (caratteri speciali con escape)":
    "LaTeX (special characters escaped)",
  "LaTeX copiato ✓":
    "LaTeX copied ✓",
  "LaTeX grezzo":
    "Raw LaTeX",
  "LaTeX riconosciuto — modificabile":
    "Recognized LaTeX — editable",
  "Larghezza pagina = finestra (W)":
    "Page width = window (W)",
  "Le formule su più righe non si possono avvolgere in \\mathrm{} (avvolgi le singole righe a mano)":
    "Multi-line formulas cannot be wrapped in \\mathrm{} (wrap the single lines by hand)",
  "Lente AI":
    "AI Lens",
  "Lente AI — {compito}":
    "AI Lens — {compito}",
  "Lente AI: Spiega / Traduci / Chiedi (con AI locale attiva)":
    "AI Lens: Explain / Translate / Ask (with the local AI enabled)",
  "Locale":
    "Local",
  "Manda il testo agli Appunti nel formato scelto (con citazione al paper)":
    "Send the text to Notes in the chosen format (with a citation to the paper)",
  "Manda la formula agli Appunti nel formato scelto (con citazione al paper)":
    "Send the formula to Notes in the chosen format (with a citation to the paper)",
  "Manda la selezione a un appunto, con citazione al paper":
    "Send the selection to a note, with a citation to the paper",
  "Manda la selezione agli Appunti, con citazione a questo paper":
    "Send the selection to Notes, with a citation to this paper",
  "Manda la tabella agli Appunti nel formato scelto (con citazione al paper)":
    "Send the table to Notes in the chosen format (with a citation to the paper)",
  "Markdown con la formattazione del PDF: *corsivo*, **grassetto**, <sup>apici</sup>, <sub>pedici</sub> — modificabile":
    "Markdown carrying the PDF's formatting: *italic*, **bold**, <sup>superscript</sup>, <sub>subscript</sub> — editable",
  "Markdown, blocco $$…$$":
    "Markdown, $$…$$ block",
  "Math-OCR locale integrato (pix2tex)":
    "Built-in local math-OCR (pix2tex)",
  "Menu radiale con i comandi di lettura":
    "Radial menu with the reading commands",
  "Migliora con AI":
    "Improve with AI",
  "Modalità nota: clicca un punto della pagina per aggiungere un appunto":
    "Note mode: click a spot on the page to add a note",
  "Modalità notte":
    "Night mode",
  "Modalità notte: inverti i colori (I)":
    "Night mode: invert the colors (I)",
  "Modalità tabella: trascina un rettangolo attorno a una tabella":
    "Table mode: drag a rectangle around a table",
  "Modello":
    "Model",
  "Modello struttura tabelle (TATR, locale): righe/colonne/intestazioni riconosciute dall'immagine, testo esatto dal PDF — il migliore per le tabelle dei paper (~111 MB al primo uso)":
    "Table-structure model (TATR, local): rows/columns/headers read from the image, exact text from the PDF — the best one for tables in papers (~111 MB on first use)",
  "Modello — ⭐ = adatto alle immagini (VLM)":
    "Model — ⭐ = good with images (VLM)",
  "Modifica nota":
    "Edit note",
  "Mostra questo aiuto":
    "Show this help",
  "Mostra/nascondi l'indice del documento":
    "Show/hide the document's table of contents",
  "Motore di estrazione":
    "Extraction engine",
  "Motore di riconoscimento":
    "Recognition engine",
  "Mouse fermo":
    "Idle mouse",
  "Nativa":
    "Native",
  "Nessun modello di visione disponibile — abilita l'AI e scarica un modello vision (es. qwen2.5vl, minicpm-v).":
    "No vision model available — enable the AI and download a vision model (e.g. qwen2.5vl, minicpm-v).",
  "Nessun testo riconosciuto nell'area selezionata. Se il PDF è scansionato, prova il motore «Ollama» (OCR con un modello di visione).":
    "No text recognized in the selected area. If the PDF is a scan, try the «Ollama» engine (OCR with a vision model).",
  "Nessun testo tabellare riconosciuto nell'area selezionata. Seleziona più precisamente attorno alla tabella, prova il motore «Ollama», oppure usa un PDF con testo (non scansionato).":
    "No tabular text recognized in the selected area. Select more tightly around the table, try the «Ollama» engine, or use a PDF with real text (not a scan).",
  "Nessuna annotazione con questi filtri.":
    "No annotations match these filters.",
  "Nessuna annotazione. Seleziona del testo nel PDF per evidenziarlo, o aggiungi una nota a un punto.":
    "No annotations yet. Select text in the PDF to highlight it, or drop a note at one spot.",
  "Nessuna figura ritagliata.":
    "No figure cropped.",
  "Nessuna formula riconosciuta. Riprova selezionando l'area più precisamente attorno all'equazione.":
    "No formula recognized. Try again, selecting the area more tightly around the equation.",
  "Niente da convertire in LaTeX":
    "Nothing to convert to LaTeX",
  "Niente da copiare":
    "Nothing to copy",
  "Niente da mandare agli Appunti":
    "Nothing to send to Notes",
  "Niente da salvare":
    "Nothing to save",
  "No":
    "No",
  "No, grazie":
    "No thanks",
  "Non riesco a ricollegare: {err}":
    "Cannot relink it: {err}",
  "Non riesco: {err}":
    "Cannot do it: {err}",
  "Nota del doc.":
    "Doc. note",
  "Nota del documento non salvata: {err}":
    "Document note not saved: {err}",
  "Nota del documento: un appunto libero su questo paper (E)":
    "Document note: a free-form note about this paper (E)",
  "Nota libera su questo documento (E)":
    "A free-form note about this document (E)",
  "Nota libera su questo documento… (salvataggio automatico)":
    "A free-form note about this document… (saved automatically)",
  "Nota non salvata: {err}":
    "Note not saved: {err}",
  "Nota puntuale":
    "Sticky note",
  "Nota puntuale, estrazione, rotazione…":
    "Sticky note, extraction, rotation…",
  "Nota…":
    "Note…",
  "Note":
    "Notes",
  "Notte":
    "Night",
  "OCR con un modello di visione locale (Ollama / LM Studio) — utile per pagine scansionate":
    "OCR with a local vision model (Ollama / LM Studio) — handy for scanned pages",
  "Pagina intera nella finestra (H)":
    "Whole page inside the window (H)",
  "Pannello delle annotazioni (A)":
    "Annotations panel (A)",
  "Più equazioni impilate → un blocco gathered":
    "Several stacked equations → one gathered block",
  "Più righe":
    "Multi-line",
  "Posizione":
    "Location",
  "Precedente (Maiusc+Invio)":
    "Previous (Shift+Enter)",
  "Primo uso: scarico il modello formula (~{mb} MB), attendi…":
    "First run: downloading the formula model (~{mb} MB), hold on…",
  "Primo uso: scarico il modello struttura tabelle (~{mb} MB), attendi…":
    "First run: downloading the table-structure model (~{mb} MB), hold on…",
  "Questo documento non ha un file da mostrare":
    "This document has no file to show",
  "Regione non disponibile":
    "The region is not available",
  "Ricollega anche l'altro":
    "Relink that one too",
  "Ricollega gli altri {n}":
    "Relink the other {n}",
  "Ricollegati anche {n} file.":
    "{n} more files relinked.",
  "Ricollegato anche 1 altro file.":
    "One more file relinked.",
  "Riconosci con un modello di visione locale (Ollama / LM Studio)":
    "Recognize with a local vision model (Ollama / LM Studio)",
  "Riconosci una formula come LaTeX: trascina un rettangolo attorno all'equazione (scorciatoia: F)":
    "Recognize a formula as LaTeX: drag a rectangle around the equation (shortcut: F)",
  "Riconosco il testo con il modello di visione…":
    "Reading the text with the vision model…",
  "Riconosco la formula con il modello di visione…":
    "Recognizing the formula with the vision model…",
  "Riconosco la formula in locale… (il primo uso scarica il modello, ~180 MB)":
    "Recognizing the formula locally… (the first run downloads the model, ~180 MB)",
  "Riduci (−)":
    "Zoom out (−)",
  "Rifinisci righe/colonne con l'AI locale (Ollama/LM Studio)":
    "Tidy up rows/columns with the local AI (Ollama/LM Studio)",
  "Risposta copiata ✓":
    "Answer copied ✓",
  "Ritaglia una figura come immagine PNG: trascina un rettangolo attorno alla figura (scorciatoia: G)":
    "Crop a figure as a PNG image: drag a rectangle around the figure (shortcut: G)",
  "Ritrova il file…":
    "Find the file…",
  "Rosa":
    "Pink",
  "Rotazione, note, estrazione tabella/testo/formula/figura, stampa, condivisione":
    "Rotation, notes, table/text/formula/figure extraction, printing, sharing",
  "Ruota":
    "Rotate",
  "Ruota a destra 90° (])":
    "Rotate 90° right (])",
  "Ruota a sinistra 90° ([)":
    "Rotate 90° left ([)",
  "Ruota dx":
    "Rotate right",
  "Ruota sx":
    "Rotate left",
  "Salva PNG…":
    "Save PNG…",
  "Salva il testo su file nel formato scelto":
    "Save the text to a file in the chosen format",
  "Salva la figura come file PNG":
    "Save the figure as a PNG file",
  "Salva la formula su file nel formato scelto":
    "Save the formula to a file in the chosen format",
  "Salva la tabella su file nel formato scelto":
    "Save the table to a file in the chosen format",
  "Salvataggio…":
    "Saving…",
  "Salvato ✓":
    "Saved ✓",
  "Salva…":
    "Save…",
  "Scorciatoie da tastiera":
    "Keyboard shortcuts",
  "Scorciatoie da tastiera (?)":
    "Keyboard shortcuts (?)",
  "Selezione testo":
    "Text selection",
  "Sottolinea":
    "Underline",
  "Sottolineati":
    "Underlines",
  "Spiega":
    "Explain",
  "Spiega la selezione con l'AI locale":
    "Explain the selection with the local AI",
  "Stampa":
    "Print",
  "Stampa non riuscita: {err}":
    "Printing failed: {err}",
  "Stampa questo documento":
    "Print this document",
  "Stampa…":
    "Printing…",
  "Stile e colore evidenziazione":
    "Highlight style and color",
  "Strumenti":
    "Tools",
  "Successivo (Invio)":
    "Next (Enter)",
  "Sì, elimina":
    "Yes, delete",
  "Tabella LaTeX in stile booktabs (nel documento serve il pacchetto booktabs)":
    "LaTeX table in booktabs style (your document needs the booktabs package)",
  "Tabella Markdown":
    "Markdown table",
  "Tabella esportata ✓":
    "Table exported ✓",
  "Tabella estratta":
    "Extracted table",
  "Tabella rifinita con AI":
    "Table tidied up with AI",
  "Tasto destro":
    "Right-click",
  "Testo copiato ✓":
    "Text copied ✓",
  "Testo estratto":
    "Extracted text",
  "Testo estratto — modificabile":
    "Extracted text — editable",
  "Testo semplice":
    "Plain text",
  "Testo semplice (formattazione rimossa). Passa a MD per vedere/correggere corsivi, apici e pedici.":
    "Plain text (formatting stripped). Switch to MD to see and fix italics, superscripts and subscripts.",
  "Togli l'involucro \\mathrm{} (torna al corsivo matematico)":
    "Remove the \\mathrm{} wrapper (back to math italics)",
  "Torna alla libreria (Esc)":
    "Back to the library (Esc)",
  "Traduci":
    "Translate",
  "Traduci la selezione con l'AI locale":
    "Translate the selection with the local AI",
  "Trascinabili dalla barra del titolo (per confrontarle con la pagina sotto) e ridimensionabili dall'angolo":
    "Draggable by their title bar (to compare them with the page underneath) and resizable from the corner",
  "Tutti i colori":
    "All colors",
  "Tutti i tipi":
    "All kinds",
  "Un altro file è nella stessa cartella":
    "One more file is in the same folder",
  "Una":
    "One",
  "Una singola formula":
    "A single formula",
  "Vai a pagina {pagina}":
    "Go to page {pagina}",
  "Vai agli Appunti":
    "Go to Notes",
  "Verde":
    "Green",
  "Vista a due pagine":
    "Two-page view",
  "Vista a due pagine (2)":
    "Two-page view (2)",
  "Zoom 100%":
    "Zoom 100%",
  "nessun risultato":
    "no results",
  "non salvate":
    "unsaved",
  "p.{pagina}":
    "p.{pagina}",
  "{i}/{tot}":
    "{i}/{tot}",
  "{i}/{tot}+":
    "{i}/{tot}+",
  "{n} evidenziazioni":
    "{n} highlights",
  "→ Nota del doc.":
    "→ Doc. note",
  "⋯ Altro":
    "⋯ More",
  "▥ Due pagine":
    "▥ Two pages",
  "⚠ LaTeX non valido — controlla la formula":
    "⚠ Invalid LaTeX — check the formula",
  "⟲ Ruota sinistra":
    "⟲ Rotate left",
  "⟳ Ruota destra":
    "⟳ Rotate right",
  "⭐ Visione (consigliati)":
    "⭐ Vision (recommended)",
  "incolla la chiave e premi Salva…":
    "paste the key and press Save…",
  "Articolo peer-reviewed (pubblicato)":
    "Peer-reviewed article (published)",
  "Preprint":
    "Preprint",
  "Preprint — nessuna versione peer-reviewed nota":
    "Preprint — no peer-reviewed version known",
  "La versione peer-reviewed esiste — apri (DOI)":
    "A peer-reviewed version exists — open it (DOI)",
  "La versione peer-reviewed esiste":
    "A peer-reviewed version exists",
  "Mostra/nascondi la barra laterale (Ctrl+B)":
    "Show/hide the sidebar (Ctrl+B)",
  "Barra laterale":
    "Sidebar",
  "Documenti mostrati (o nel filtro attivo)":
    "Documents on screen (or inside the active filter)",
  "AI locale disattivata — clic per riattivarla (riassunti, tag automatici, domande…)":
    "Local AI is off — click to turn it back on (summaries, automatic tags, questions…)",
  "AI locale disattivata — clic per riattivarla":
    "Local AI is off — click to turn it back on",
  "Indice semantico: abilita ricerca per significato, correlati e costellazione":
    "Semantic index: powers search by meaning, Related and the Constellation",
  "Indice semantico":
    "Semantic index",
  "Cerca per testo o significato…  ( / )":
    "Search by text or by meaning…  ( / )",
  "Cerca nei tuoi PDF — scorciatoia: /":
    "Search inside your PDFs — shortcut: /",
  "Come cercare: Tutto (testo + significato), Testo (parole esatte) o Semantica (per significato)":
    "How to search: All (text + meaning), Text (exact words) or Semantic (by meaning)",
  "Cerca online la copia Open Access delle voci senza PDF (arXiv, Unpaywall, OpenAlex, Semantic Scholar). Scarica solo quando la corrispondenza col titolo è certa. Richiede la ricerca online attiva.":
    "Look online for the Open Access copy of the entries with no PDF (arXiv, Unpaywall, OpenAlex, Semantic Scholar). It only downloads when the title matches for certain. Requires online search to be on.",
  "Palette comandi — ogni azione, digitando (Ctrl+K)":
    "Command palette — every action, by typing (Ctrl+K)",
  "Griglia (copertine)":
    "Grid (covers)",
  "Vista a griglia":
    "Grid view",
  "Lista (colonne ordinabili)":
    "List (sortable columns)",
  "Vista a lista":
    "List view",
  "Costellazione: la libreria come mappa semantica":
    "Constellation: your library as a semantic map",
  "Vista a costellazione":
    "Constellation view",
  "Dimensione delle copertine nella griglia":
    "Cover size in the grid",
  "Copertine più piccole":
    "Smaller covers",
  "Più piccole":
    "Smaller",
  "Dimensione copertine":
    "Cover size",
  "Copertine più grandi":
    "Bigger covers",
  "Più grandi":
    "Bigger",
  "Vista":
    "View",
  "Seleziona o deseleziona tutti i documenti mostrati (per le azioni multiple)":
    "Select or deselect every document on screen (for bulk actions)",
  "Ordina i documenti (più criteri combinabili)":
    "Sort the documents (criteria can be combined)",
  "Azzera l'ordinamento":
    "Clear the sort order",
  "Ferma l'indicizzazione (i documenti già indicizzati restano salvati)":
    "Stop indexing (the documents already indexed stay saved)",
  "Calcola gli embedding mancanti (la prima volta scarica il modello ~2.3GB)":
    "Compute the missing embeddings (the first run downloads the ~2.3 GB model)",
  "Mostra tutti i documenti (rimuovi i filtri)":
    "Show every document (drop the filters)",
  "Solo i documenti contrassegnati come preferiti":
    "Only the documents marked as favorites",
  "Solo i documenti non ancora segnati come letti":
    "Only the documents not yet marked as read",
  "Solo i documenti che citano un repository GitHub (codice disponibile)":
    "Only the documents that point to a GitHub repository (code available)",
  "Solo gli articoli peer-reviewed (pubblicati), esclusi i preprint":
    "Only peer-reviewed (published) articles, preprints excluded",
  "I tuoi lavori, importati da progetti LaTeX (.zip)":
    "Your own work, imported from LaTeX projects (.zip)",
  "Comprimi o espandi i tag":
    "Collapse or expand the tags",
  "Ordina i tag in ordine alfabetico (A→Z / Z→A)":
    "Sort the tags alphabetically (A→Z / Z→A)",
  "Azzera il filtro per tag":
    "Clear the tag filter",
  "Filtra per il tag «{nome}» (1 paper) — puoi selezionarne più di uno":
    "Filter by the tag “{nome}” (1 paper) — you can pick more than one",
  "Filtra per il tag «{nome}» ({n} paper) — puoi selezionarne più di uno":
    "Filter by the tag “{nome}” ({n} papers) — you can pick more than one",
  "Rinomina o cambia colore":
    "Rename or recolor",
  "Modifica il tag {nome}":
    "Edit the tag {nome}",
  "Elimina questo tag (lo rimuove da tutti i documenti)":
    "Delete this tag (removes it from every document)",
  "Archivio: raccolte e sotto-raccolte ad albero, trascinamento, suggerimenti":
    "Archive: collections and sub-collections as a tree, drag and drop, suggestions",
  "Filtra per la raccolta «{nome}» (smart: si aggiorna da sola)":
    "Filter by the collection “{nome}” (smart: it updates itself)",
  "Filtra per la raccolta «{nome}»":
    "Filter by the collection “{nome}”",
  "Costellazione della sola raccolta «{nome}»":
    "Constellation of the collection “{nome}” alone",
  "Costellazione di {nome}":
    "Constellation of {nome}",
  "Elimina la raccolta (i documenti restano in libreria)":
    "Delete the collection (the documents stay in your library)",
  "Nuova raccolta…":
    "New collection…",
  "Nome della nuova raccolta. Premi Invio o «Crea»":
    "Name of the new collection. Press Enter or “Create”",
  "Raccolta automatica: si popola da sola in base a una regola, invece di aggiungere i documenti a mano":
    "Automatic collection: it fills itself from a rule, instead of you adding documents by hand",
  "Regola di appartenenza della raccolta smart":
    "Membership rule for the smart collection",
  "valore":
    "value",
  "Valore della regola: anno minimo, id del tag, o testo da cercare":
    "Value for the rule: earliest year, tag id, or text to look for",
  "Crea la raccolta":
    "Create the collection",
  "Rilancia «{nome}» e mostra le novità — fonte: {fonte}":
    "Run “{nome}” again and show what is new — source: {fonte}",
  "Elimina questa ricerca salvata":
    "Delete this saved search",
  "Smetti di sorvegliare":
    "Stop watching",
  "Scegli una cartella: importa subito i PDF già presenti e poi quelli che aggiungerai automaticamente":
    "Pick a folder: the PDFs already in it are imported now, and whatever you drop in later follows automatically",
  "Chiedi alla libreria, Wiki, Appunti, Cerca online, Novità, Cura della libreria, Cestino, Terminale, Guida, Impostazioni e Informazioni sono ora nella barra strumenti in alto ↑ — tasto destro: menu radiale · Ctrl+K: palette":
    "Ask your library, Wiki, Notes, Search online, What's new, Library care, Trash, Terminal, Guide, Settings and About now live in the toolbar at the top ↑ — right-click: radial menu · Ctrl+K: palette",
  "Elimina definitivamente tutto il cestino":
    "Permanently delete everything in the trash",
  "Ripristina nella libreria":
    "Restore to the library",
  "Elimina definitivamente (irreversibile)":
    "Delete permanently (cannot be undone)",
  "Unisci nel primo: sposta tag/raccolte/annotazioni, gli altri finiscono nel cestino":
    "Merge into the first one: tags, collections and annotations move over, the others go to the trash",
  "Crea/aggiorna l'indice dei passaggi (necessario per le risposte)":
    "Build/update the passage index (answers need it)",
  "Rigenera tutti i passaggi da zero (per ottenere le pagine sui documenti già indicizzati)":
    "Rebuild every passage from scratch (to get page numbers on documents already indexed)",
  "Cerca in tutta la libreria":
    "Search the whole library",
  "Apri la fonte":
    "Open the source",
  "Passaggio usato (p. {pagina}):\n\n{testo}":
    "Passage used (p. {pagina}):\n\n{testo}",
  "Passaggio usato:\n\n{testo}":
    "Passage used:\n\n{testo}",
  "Nuova pagina: concetto o tag…":
    "New page: concept or tag…",
  "Scrivi un concetto (o il nome di un tag): la pagina viene sintetizzata dai documenti pertinenti":
    "Type a concept (or a tag name): the page is synthesized from the documents that fit it",
  "Una pagina per ogni tag con almeno 2 documenti (le esistenti vengono rigenerate)":
    "One page per tag with at least 2 documents (existing ones are regenerated)",
  "Ferma al prossimo passaggio":
    "Stop at the next step",
  "1 fonte":
    "1 source",
  "{n} fonti":
    "{n} sources",
  "generata {quando}":
    "generated {quando}",
  "la libreria è cambiata: rigenera":
    "the library has changed: regenerate",
  "La libreria è cambiata da quando è stata generata":
    "The library has changed since this page was generated",
  "Elimina questa pagina":
    "Delete this page",
  "Rigenera la pagina con lo stato attuale della libreria":
    "Regenerate the page from the library as it is now",
  "Nuovo appunto: titolo…":
    "New note: title…",
  "Crea un nuovo appunto .md":
    "Create a new .md note",
  "Crea l'appunto":
    "Create the note",
  "Apri la cartella degli appunti (.md) nel file explorer":
    "Open the notes folder (.md) in File Explorer",
  "Ordina l'elenco degli appunti":
    "Sort the list of notes",
  "Ultima modifica · creazione":
    "Last modified · created",
  "Elimina questo appunto":
    "Delete this note",
  "Nuovo titolo — Invio per confermare, Esc o clic fuori per annullare (rinomina anche il file .md)":
    "New title — Enter to confirm, Esc or a click outside to cancel (the .md file is renamed too)",
  "Doppio clic per rinominare":
    "Double-click to rename",
  "Rinomina l'appunto: cambia il titolo e il nome del file":
    "Rename the note: changes both the title and the file name",
  "Modifica il Markdown":
    "Edit the Markdown",
  "Modifica con anteprima affiancata in tempo reale":
    "Edit with a live preview side by side",
  "Anteprima resa (formule, collegamenti, immagini)":
    "Rendered preview (formulas, links, images)",
  "Salva una copia .md dell'appunto (Markdown puro)":
    "Save a .md copy of the note (plain Markdown)",
  "Esporta come pagina HTML autonoma (formule in MathML e immagini incluse)":
    "Export as a self-contained HTML page (MathML formulas and images included)",
  "Esporta come documento LaTeX (.tex con le figure estratte in una cartella)":
    "Export as a LaTeX document (.tex, with the figures pulled out into a folder)",
  "Apre la stampa dell'appunto reso: scegli «Salva come PDF»":
    "Opens the print dialog on the rendered note: choose “Save as PDF”",
  "Data di creazione del file":
    "When the file was created",
  "Ultima modifica":
    "Last modified",
  "Formattazione":
    "Formatting",
  "Titolo (H1)":
    "Heading (H1)",
  "Sottotitolo (H2)":
    "Subheading (H2)",
  "Titolo minore (H3)":
    "Minor heading (H3)",
  "Grassetto":
    "Bold",
  "Corsivo":
    "Italic",
  "Codice inline":
    "Inline code",
  "Collegamento":
    "Link",
  "Elenco puntato":
    "Bulleted list",
  "Elenco numerato":
    "Numbered list",
  "Citazione":
    "Blockquote",
  "Blocco formula LaTeX ($$…$$)":
    "LaTeX formula block ($$…$$)",
  "Separatore orizzontale":
    "Horizontal rule",
  "Sposta su il blocco (paragrafo/immagine)":
    "Move the block up (paragraph/image)",
  "Sposta giù il blocco (paragrafo/immagine)":
    "Move the block down (paragraph/image)",
  "Apri «{titolo}»":
    "Open “{titolo}”",
  "Cerca subito nuovi paper per tutte le ricerche salvate":
    "Look for new papers right now, across every saved search",
  "Segna tutte come lette (le rimuove dalle novità)":
    "Mark them all as read (drops them from What's new)",
  "Mostra/nascondi abstract":
    "Show/hide the abstract",
  "Già presente in libreria":
    "Already in your library",
  "Rimuovi dalle novità":
    "Remove from What's new",
  "Aggiungi alla libreria (scarica il PDF se Open Access)":
    "Add to the library (downloads the PDF if it is Open Access)",
  "Fonte di ricerca":
    "Search source",
  "Cerca paper online…":
    "Search for papers online…",
  "autore":
    "author",
  "Filtra per autore (opzionale)":
    "Filter by author (optional)",
  "dal":
    "from",
  "al":
    "to",
  "Anno minimo":
    "Earliest year",
  "Anno massimo":
    "Latest year",
  "Mostra solo lavori Open Access":
    "Show Open Access work only",
  "Ordina i risultati":
    "Sort the results",
  "Salva questa ricerca per monitorare le novità sul tema":
    "Save this search to keep watching the topic",
  "Mostra solo i paper che pubblicano anche il codice (repository GitHub rilevato)":
    "Show only papers that also publish their code (a GitHub repository was detected)",
  "Mostra solo i paper peer-reviewed (esiste una versione pubblicata)":
    "Show only peer-reviewed papers (a published version exists)",
  "Mostra solo i preprint":
    "Show preprints only",
  "Rimuovi tutti i filtri":
    "Drop every filter",
  "Ordina per titolo":
    "Sort by title",
  "Ordina per anno":
    "Sort by year",
  "Ordina per rivista":
    "Sort by journal",
  "Ordina per citazioni":
    "Sort by citations",
  "azioni":
    "actions",
  "Già nella libreria":
    "Already in your library",
  "Nuovo dall'ultima volta che hai eseguito questa ricerca salvata":
    "New since the last time you ran this saved search",
  "Codice su GitHub: {url}":
    "Code on GitHub: {url}",
  "Repository GitHub":
    "GitHub repository",
  "Apri la pagina del paper nel browser":
    "Open the paper's page in your browser",
  "Rimuovi il filtro e mostra tutto":
    "Drop the filter and show everything",
  "Rimuovi questo tag dal filtro":
    "Remove this tag from the filter",
  "Mostra i documenti che hanno TUTTI i tag selezionati":
    "Show the documents that carry ALL the selected tags",
  "Mostra i documenti che hanno ALMENO UNO dei tag selezionati":
    "Show the documents that carry AT LEAST ONE of the selected tags",
  "Rimuovi il filtro per tag":
    "Drop the tag filter",
  "Azioni sulla selezione":
    "Actions on the selection",
  "Stampa i documenti selezionati come un unico lavoro di stampa":
    "Print the selected documents as a single print job",
  "Genera un riassunto AI per ogni documento selezionato":
    "Generate an AI summary for each selected document",
  "Genera tag automatici AI per ogni documento selezionato":
    "Generate automatic AI tags for each selected document",
  "Aggiungi un tag ai selezionati":
    "Add a tag to the selected documents",
  "Aggiungi i selezionati a una raccolta":
    "Add the selected documents to a collection",
  "Sposta i selezionati nel cestino":
    "Move the selected documents to the trash",
  "Tutte le azioni sulla selezione: Trova PDF, Confronta, Rassegna, Cita, Esporta…":
    "Every action on the selection: Find PDF, Compare, Review, Cite, Export…",
  "Annulla la selezione":
    "Clear the selection",
  "Deseleziona":
    "Clear selection",
  "Vai ai documenti da leggere":
    "Go to the documents still to read",
  "Documenti aperti ma non ancora finiti":
    "Documents opened but not finished",
  "Aggiunti alla libreria questo mese":
    "Added to the library this month",
  "Un altro paper a caso":
    "Another paper at random",
  "Un altro":
    "Another one",
  "Apri il documento alla pagina {pagina}":
    "Open the document at page {pagina}",
  "Altre azioni (anche col tasto destro)":
    "More actions (right-click works too)",
  "Seleziona":
    "Select",
  "Seleziona per azioni multiple":
    "Select for bulk actions",
  "Letto al {pct}% (pag. {p}/{tot})":
    "{pct}% read (p. {p}/{tot})",
  "Mostra tutti i lavori di {autore}":
    "Show everything by {autore}",
  "Citekey: {citekey} — clic per copiare":
    "Citekey: {citekey} — click to copy",
  "Copia citekey {citekey}":
    "Copy citekey {citekey}",
  "Riassunto AI già presente — lo trovi in «Modifica metadati» (il batch AI salta questo documento)":
    "An AI summary already exists — you will find it under “Edit metadata” (the AI batch skips this document)",
  "Riassunto AI già presente (il batch AI salta questo documento)":
    "An AI summary already exists (the AI batch skips this document)",
  "1 evidenziazione: cercala da qui, la ricerca guarda anche dentro le tue note sul PDF":
    "1 highlight: search it from here — search also looks inside the notes you wrote on the PDF",
  "{n} evidenziazioni: cercale da qui, la ricerca guarda anche dentro le tue note sul PDF":
    "{n} highlights: search them from here — search also looks inside the notes you wrote on the PDF",
  "Cerca online la scheda giusta per QUESTO documento e scegli tu quale applicare":
    "Look online for the right record for THIS document, and you pick which one to apply",
  "Recupera i metadati di questo documento (scegli tu la scheda giusta)":
    "Fetch the metadata for this document (you pick the right record)",
  "Apri il repository GitHub: {url}":
    "Open the GitHub repository: {url}",
  "Apri repository GitHub":
    "Open the GitHub repository",
  "Filtra: mostra solo i paper col tag «{nome}» (clicca altri tag per restringere; ri-clic per togliere)":
    "Filter: show only the papers tagged “{nome}” (click more tags to narrow down; click again to drop one)",
  "Ordina per titolo (clicca di nuovo per invertire, ancora per togliere)":
    "Sort by title (click again to reverse, once more to drop it)",
  "Ordina per primo autore (clicca di nuovo per invertire, ancora per togliere)":
    "Sort by first author (click again to reverse, once more to drop it)",
  "Ordina per anno (clicca di nuovo per invertire, ancora per togliere)":
    "Sort by year (click again to reverse, once more to drop it)",
  "Ordina per rivista (clicca di nuovo per invertire, ancora per togliere)":
    "Sort by journal (click again to reverse, once more to drop it)",
  "Ordina per data di aggiunta (clicca di nuovo per invertire, ancora per togliere)":
    "Sort by date added (click again to reverse, once more to drop it)",
  "Suggerimento iniziale":
    "Getting-started tip",
  "nuovo tag…":
    "new tag…",
  "Apri il documento":
    "Open the document",
  "Le [n] diventano [@citekey]":
    "The [n] markers become [@citekey]",
  "Crea un appunto .md, con le fonti come backlink [[@citekey]] cliccabili":
    "Create an .md note, with the sources as clickable [[@citekey]] backlinks",
  "Tabella risultati":
    "Results table",
  "Percorso di lettura":
    "Reading path",
  "Apri il DOI nel browser":
    "Open the DOI in your browser",
  "Aggiungi come riferimento alla libreria":
    "Add to the library as a reference",
  "Pagina wiki dalla selezione":
    "Wiki page from the selection",
  "Titolo del concetto (es. «ragionamento negli LLM»)…":
    "Title of the concept (e.g. “reasoning in LLMs”)…",
  "Riferimento senza PDF":
    "Reference with no PDF",
  "Cerca i candidati online — arXiv, Unpaywall, OpenAlex, Semantic Scholar, Crossref, per identificativo e per titolo — e scegli tu quale scaricare e allegare":
    "Look for candidates online — arXiv, Unpaywall, OpenAlex, Semantic Scholar, Crossref, by identifier and by title — and you pick which one to download and attach",
  "Incolla il link dagli appunti":
    "Paste the link from the clipboard",
  "Ignora questo link":
    "Dismiss this link",
  "Ignora":
    "Dismiss",
  "Interrompi l'operazione AI in corso":
    "Stop the AI job that is running",
  "Interrompi il recupero (quanto già aggiornato resta)":
    "Stop the fetch (whatever is already updated is kept)",
  "Interrompi la ricerca dei PDF (quelli già allegati restano)":
    "Stop the PDF hunt (the ones already attached are kept)",
  "Copia il messaggio":
    "Copy the message",
  "Incolla dagli appunti":
    "Paste from the clipboard",
  "Aggiornamento disponibile":
    "Update available",
  "Novità di questa versione":
    "What's new in this version",
  "È disponibile Scriptorium {nuova} — clic per vedere le novità e installare":
    "Scriptorium {nuova} is out — click to see what's new and install it",
  "È disponibile Scriptorium {nuova} — apri GitHub":
    "Scriptorium {nuova} is out — open GitHub",
  "Nuova versione {nuova} disponibile":
    "New version {nuova} available",
  "Carico il modello bge-m3…":
    "Loading the bge-m3 model…",
  "Indicizzo {fatti}/{totale}":
    "Indexing {fatti}/{totale}",
  "Apri questo documento":
    "Open this document",
  "Già in libreria":
    "Already in your library",
  "Nella tua libreria — apri":
    "In your library — open it",
  "Non in libreria":
    "Not in your library",
  "Apri il DOI":
    "Open the DOI",
  "Aggiungi alla libreria (scarica il PDF se Open Access, altrimenti come riferimento)":
    "Add to the library (downloads the PDF if it is Open Access, otherwise adds it as a reference)",
  "Aggiungi questo paper col PDF che stai guardando nel browser: apri il PDF (↗), copia il suo link e incollalo qui":
    "Add this paper with the PDF you are looking at in the browser: open the PDF (↗), copy its link and paste it here",
  "Esplora le citazioni di questo paper":
    "Explore this paper's citations",
  "Apri la pagina del paper":
    "Open the paper's page",
  "incolla il link diretto al PDF (https://…)":
    "paste the direct link to the PDF (https://…)",
  "Scarica e aggiungi col PDF":
    "Download and add it with the PDF",
  "Torna al paper precedente":
    "Back to the previous paper",
  "Vista esplorazione":
    "Exploration view",
  "Mappa temporale: riferimenti a sinistra, citazioni a destra":
    "Timeline map: references on the left, citations on the right",
  "Liste con tutte le azioni (+ PDF, salva…)":
    "Lists with every action (+ PDF, save…)",
  "Salva questa lista (con i link ai paper) in un file Markdown":
    "Save this list (with links to the papers) to a Markdown file",
  "Scarica il PDF se Open Access, altrimenti aggiunge come riferimento":
    "Downloads the PDF if it is Open Access, otherwise adds it as a reference",
  "Ricentra la mappa su questo paper (← Indietro per tornare)":
    "Recenter the map on this paper (← Back to return)",
  "Riscopri":
    "Rediscover",
  "nome del tag":
    "tag name",
  "Nome del tag":
    "Tag name",
  "Colore {colore}":
    "Color {colore}",
  "Sezioni":
    "Sections",
  "Riconosci il testo della scansione (motore OCR di Windows) e rendilo cercabile":
    "Recognize the text of the scan (Windows OCR engine) and make it searchable",
  "Cerca online la scheda giusta (Crossref, arXiv, OpenAlex) e confermala tu":
    "Look online for the right record (Crossref, arXiv, OpenAlex) and confirm it yourself",
  "Indica dov'è finito il file: la scheda con appunti, evidenziazioni e tag resta la stessa":
    "Point to where the file ended up: the record keeps its notes, highlights and tags",
  "Cerca online (Crossref) un DOI per i riferimenti che ne sono privi, così entrano nel conteggio dei gap":
    "Look online (Crossref) for a DOI for the references that have none, so they count towards the gaps",
  "Citato da 1 tuo documento":
    "Cited by 1 of your documents",
  "Citato da {n} tuoi documenti":
    "Cited by {n} of your documents",
  "Cerca questo DOI online per aggiungerlo":
    "Search for this DOI online so you can add it",
  "Copia il DOI":
    "Copy the DOI",
  "Apri su Hugging Face":
    "Open on Hugging Face",
  "Guida a Scriptorium":
    "Scriptorium guide",
  "Trascina per spostare la guida":
    "Drag to move the guide",
  "Tieni la guida sopra ogni altra vista, anche il lettore":
    "Keep the guide above every other view, the reader included",
  "Chiudi la guida":
    "Close the guide",
  "tua@email.it":
    "you@email.com",
  "Avvia il server Ollama (ollama serve)":
    "Start the Ollama server (ollama serve)",
  "Ferma il server Ollama":
    "Stop the Ollama server",
  "Avvia il server di LM Studio (lms server start)":
    "Start the LM Studio server (lms server start)",
  "Ferma il server di LM Studio (lms server stop)":
    "Stop the LM Studio server (lms server stop)",
  "(nessuna cartella scelta)":
    "(no folder chosen)",
  "Trascinami nella barra dei preferiti del browser":
    "Drag me into your browser's bookmarks bar",
  "Quando torni su Scriptorium, se hai copiato un link che sembra un PDF compare un suggerimento «Aggancia»":
    "When you come back to Scriptorium, if you have copied a link that looks like a PDF you get a “Grab it” prompt",
  "in primo piano":
    "always on top",
  "FAQ":
    "FAQ",
  "Gestore locale di PDF, riferimenti e appunti: tutto resta sul tuo computer, le funzioni di rete e AI sono opzionali. <strong>Regola d'oro</strong>: qualunque cosa cerchi, premi <kbd>Ctrl</kbd>+<kbd>K</kbd> e digitala.":
    "A local manager for PDFs, references and notes: everything stays on your computer, and the network and AI features are optional. <strong>Golden rule</strong>: whatever you are after, press <kbd>Ctrl</kbd>+<kbd>K</kbd> and type it.",
  "✓ impostata":
    "✓ set",
  "Sostituisci":
    "Replace",
  "cerco…":
    "searching…",
  "recupero… {fatti}/{totale}":
    "recovering… {fatti}/{totale}",
  "recupero…":
    "recovering…",
  "✦ {n} senza metadati":
    "✦ {n} without metadata",
  "cerco PDF… {fatti}/{totale}":
    "looking for PDFs… {fatti}/{totale}",
  "✦ {n} senza PDF":
    "✦ {n} without a PDF",
  "Deseleziona tutti":
    "Deselect all",
  "Seleziona tutti":
    "Select all",
  "Ordina per":
    "Sort by",
  "un tocco attiva · un altro inverte · un terzo toglie":
    "one tap turns it on · another reverses it · a third drops it",
  "azzera":
    "clear",
  "Abilita la ricerca per significato, i «Correlati» e la Costellazione. {fatti}/{totale} documenti indicizzati.":
    "Powers search by meaning, “Related” and the Constellation. {fatti}/{totale} documents indexed.",
  "Carico modello bge-m3…":
    "Loading the bge-m3 model…",
  "Stop":
    "Stop",
  "Aggiornato ✓":
    "Up to date ✓",
  "Genera":
    "Generate",
  "Raccolte":
    "Collections",
  "gestisci →":
    "manage →",
  "Senza tag":
    "Untagged",
  "Anno ≥":
    "Year ≥",
  "Per tag (id)":
    "By tag (id)",
  "Ricerche salvate":
    "Saved searches",
  "Cartella sorvegliata":
    "Watched folder",
  "Scegli cartella…":
    "Choose a folder…",
  "Gli strumenti sono nella barra in alto ↑":
    "The tools are in the bar at the top ↑",
  "Cestino — 1 elemento":
    "Trash — 1 item",
  "Cestino — {n} elementi":
    "Trash — {n} items",
  "Cestino vuoto":
    "The trash is empty",
  "Ripristina":
    "Restore",
  "Duplicati — 1 gruppo":
    "Duplicates — 1 group",
  "Duplicati — {n} gruppi":
    "Duplicates — {n} groups",
  "Nessun duplicato":
    "No duplicates",
  "Nessun doppione per DOI o titolo+anno.":
    "Nothing doubled up by DOI or by title+year.",
  "Domande in linguaggio naturale → risposta con citazioni dai tuoi documenti. Tutto in locale (recupero dei passaggi + espansione su citazioni e documenti simili + AI locale).":
    "Questions in plain language → an answer with citations from your own documents. All of it local (passage retrieval + expansion over citations and similar documents + local AI).",
  "Indice: {fatti}/{totale} documenti · {passaggi} passaggi":
    "Index: {fatti}/{totale} documents · {passaggi} passages",
  "Indicizzazione… {fatti}/{totale}":
    "Indexing… {fatti}/{totale}",
  "Indicizzazione…":
    "Indexing…",
  "Costruisci/aggiorna indice":
    "Build/update the index",
  "Aggiorna indice":
    "Update the index",
  "Le funzioni AI sono disattivate. Abilitale in":
    "The AI features are off. Turn them on in",
  "Impostazioni → AI locale":
    "Settings → Local AI",
  "(serve Ollama o LM Studio).":
    "(you need Ollama or LM Studio).",
  "Provider AI non raggiungibile. Avvia Ollama/LM Studio o controlla le Impostazioni.":
    "The AI provider is unreachable. Start Ollama/LM Studio, or check the Settings.",
  "Ambito:":
    "Scope:",
  "Sto cercando nei tuoi documenti…":
    "Searching your documents…",
  "Fonti":
    "Sources",
  "p. {pagina}":
    "p. {pagina}",
  "Genero…":
    "Generating…",
  "Genera/aggiorna dai tag":
    "Generate/update from tags",
  "Leggo le fonti {fatti}/{totale}":
    "Reading the sources {fatti}/{totale}",
  "Scrivo la pagina…":
    "Writing the page…",
  "Controllo le fonti…":
    "Checking the sources…",
  "{fase} — {concetto}":
    "{fase} — {concetto}",
  "Le funzioni AI sono disattivate: abilitale in":
    "The AI features are off: turn them on in",
  "Nessuna pagina ancora. Scrivi un concetto qui sopra, o parti da «Genera dai tag».":
    "No pages yet. Type a concept above, or start from “Generate from tags”.",
  "Rigenera":
    "Regenerate",
  "[{n}] {titolo}":
    "[{n}] {titolo}",
  " ({anno})":
    " ({anno})",
  "La tua enciclopedia privata":
    "Your private encyclopedia",
  "Ogni pagina è scritta dall'AI locale leggendo <strong>solo i tuoi documenti</strong>: le citazioni [n] aprono il PDF alla pagina giusta, i concetti si collegano tra loro, e nulla esce dal tuo computer.":
    "Every page is written by the local AI reading <strong>only your own documents</strong>: the [n] citations open the PDF at the right page, concepts link to one another, and nothing leaves your computer.",
  "Suggerimento: parti da «Genera/aggiorna dai tag» — una pagina per ciascun tema della tua libreria.":
    "Tip: start from “Generate/update from tags” — one page per theme in your library.",
  "Nuovo":
    "New",
  "Apri cartella appunti":
    "Open the notes folder",
  "Data creazione":
    "Date created",
  "Titolo (A→Z)":
    "Title (A→Z)",
  "mod. {modificato} · creato {creato}":
    "mod. {modificato} · created {creato}",
  "Nessun appunto. Scrivi un titolo qui sopra e premi «Nuovo». Gli appunti sono file .md su disco: collega con [[Titolo appunto]] oppure [[@citekey]] per un paper.":
    "No notes yet. Type a title above and press “New”. Notes are real .md files on disk: link with [[Note title]], or [[@citekey]] for a paper.",
  "Salvo…":
    "Saving…",
  "Rinomina":
    "Rename",
  "Modifica":
    "Edit",
  "Affiancato":
    "Side by side",
  "Anteprima":
    "Preview",
  "creata {quando}":
    "created {quando}",
  "modificata {quando}":
    "modified {quando}",
  "Collegato da":
    "Linked from",
  "I tuoi appunti":
    "Your notes",
  "Appunti in <strong>Markdown</strong>, salvati come <strong>file .md veri</strong> nella cartella degli appunti — restano tuoi, leggibili e modificabili anche da terminale o da qualsiasi editor.":
    "Notes in <strong>Markdown</strong>, saved as <strong>real .md files</strong> in the notes folder — they stay yours, readable and editable from a terminal or from any editor.",
  "Collega con <code>[[Titolo appunto]]</code> o un paper con <code>[[@citekey]]</code> / <code>[[Titolo del paper]]</code>. I backlink compaiono in fondo a ogni appunto.":
    "Link with <code>[[Note title]]</code>, or a paper with <code>[[@citekey]]</code> / <code>[[Paper title]]</code>. Backlinks show up at the bottom of every note.",
  "Scrivi <code>$$ … $$</code> per una <strong>formula</strong> (resa in anteprima), usa la barra di formattazione (grassetto, titoli, liste, sposta blocchi), <strong>trascina o incolla immagini</strong> (salvate come file in <code>assets/</code>, nell'appunto resta solo un riferimento breve), ed <strong>esporta</strong> in <strong>HTML</strong>, <strong>LaTeX</strong> o <strong>PDF</strong> con formule e figure incluse.":
    "Write <code>$$ … $$</code> for a <strong>formula</strong> (rendered in the preview), use the formatting bar (bold, headings, lists, move blocks), <strong>drag or paste images</strong> (saved as files under <code>assets/</code>, the note itself keeps only a short reference), and <strong>export</strong> to <strong>HTML</strong>, <strong>LaTeX</strong> or <strong>PDF</strong> with formulas and figures included.",
  "Nuovi paper sui temi che segui — raccolti a ogni avvio dalle tue ricerche salvate":
    "New papers on the topics you follow — collected at every start-up from your saved searches",
  "Cerco…":
    "Searching…",
  "↻ Cerca ora":
    "↻ Search now",
  "Carico le novità…":
    "Loading what is new…",
  "La tua ricerca salvata viene ricontrollata a ogni avvio. Quando esce qualcosa di nuovo lo trovi qui, pronto da aggiungere con un click.":
    "Your saved search is re-checked at every start-up. When something new comes out you find it here, ready to add with one click.",
  "Le tue {n} ricerche salvate vengono ricontrollate a ogni avvio. Quando esce qualcosa di nuovo lo trovi qui, pronto da aggiungere con un click.":
    "Your {n} saved searches are re-checked at every start-up. When something new comes out you find it here, ready to add with one click.",
  "Puoi forzare il controllo adesso con «↻ Cerca ora».":
    "You can force a check right now with “↻ Search now”.",
  "Salva una ricerca da <strong>Cerca online</strong> (★ Salva) per iniziare a monitorare un tema: le novità compariranno qui.":
    "Save a search from <strong>Search online</strong> (★ Save) to start watching a topic: new papers will show up here.",
  "segna tutte lette":
    "mark all as read",
  "in libreria":
    "in library",
  "Aggiungi":
    "Add",
  "HF Papers (ex Papers with Code)":
    "HF Papers (formerly Papers with Code)",
  "Solo OA":
    "OA only",
  "Rilevanza":
    "Relevance",
  "Data":
    "Date",
  "★ Salva":
    "★ Save",
  "Cerca paper online":
    "Find papers online",
  "Fonti: arXiv (preprint STEM), OpenAlex (tutto), ADS (astrofisica), Semantic Scholar (citazioni), Europe PMC (biomedicina), CORE (full-text OA), DOAJ (riviste OA), HF Papers (paper con codice — il successore di Papers with Code). I PDF si scaricano solo se Open Access; gli altri si aggiungono come riferimento.":
    "Sources: arXiv (STEM preprints), OpenAlex (everything), ADS (astrophysics), Semantic Scholar (citations), Europe PMC (biomedicine), CORE (OA full text), DOAJ (OA journals), HF Papers (papers with code — the successor to Papers with Code). PDFs are downloaded only when Open Access; the rest are added as references.",
  "Filtra:":
    "Filter:",
  "Con codice":
    "With code",
  "✕ azzera":
    "✕ clear",
  "{mostrati} / {totale} mostrati":
    "{mostrati} / {totale} shown",
  "Autori":
    "Authors",
  "Cit.":
    "Cited",
  "novità":
    "new",
  "Nessun risultato con i filtri attivi.":
    "No results with these filters on.",
  "Azzera i filtri":
    "Clear the filters",
  "Correlati a":
    "Related to",
  "Raccolta":
    "Collection",
  "{categoria}: ":
    "{categoria}: ",
  "Mostra tutti":
    "Show all",
  "Filtro tag:":
    "Tag filter:",
  "tutti":
    "all",
  "qualsiasi":
    "any",
  "Azzera":
    "Clear",
  "1 selezionato":
    "1 selected",
  "{n} selezionati":
    "{n} selected",
  "Riassunto {fatti}/{totale}…":
    "Summary {fatti}/{totale}…",
  "Tag {fatti}/{totale}…":
    "Tags {fatti}/{totale}…",
  "+ Tag…":
    "+ Tag…",
  "+ Raccolta…":
    "+ Collection…",
  "Panoramica":
    "Overview",
  "in lettura":
    "reading",
  "questo mese":
    "this month",
  "Continua a leggere":
    "Keep reading",
  "Appunti ({n})":
    "Notes ({n})",
  "Evidenziazioni ({n})":
    "Highlights ({n})",
  "{titolo} · p. {pagina}":
    "{titolo} · p. {pagina}",
  "1 file non importato o senza testo:":
    "1 file not imported, or with no text:",
  "{n} file non importati o senza testo:":
    "{n} files not imported, or with no text:",
  "…e altri {n}":
    "…and {n} more",
  "Costellazione della raccolta|davanti al nome della raccolta":
    "Constellation of the collection",
  "— vicinanze calcolate solo qui dentro":
    "— neighbourhoods computed inside it alone",
  "Torna a tutta la libreria":
    "Back to the whole library",
  "Nessun documento":
    "No documents",
  "Ma qui sopra ci sono altre corrispondenze:":
    "But there are other matches above:",
  "1 appunto":
    "1 note",
  "{n} appunti":
    "{n} notes",
  "Nessun risultato":
    "No results",
  "Prova un'altra ricerca o cambia modalità.":
    "Try another search, or switch mode.",
  "Nessun documento con questi tag":
    "No documents with these tags",
  "Prova a togliere un tag o passa a «qualsiasi», oppure premi «Azzera».":
    "Try dropping a tag or switching to “any”, or press “Clear”.",
  "Prova a togliere un tag, oppure premi «Azzera».":
    "Try dropping a tag, or press “Clear”.",
  "Vuoto":
    "Empty",
  "Nessun documento in «{dove}».":
    "No documents in “{dove}”.",
  "Non riesco a caricare l'elenco":
    "The list will not load",
  "La libreria è vuota":
    "The library is empty",
  "Trascina qui dei PDF, oppure comincia da una di queste porte:":
    "Drag some PDFs in here, or start from one of these doors:",
  "Importa PDF dal disco…":
    "Import PDFs from disk…",
  "Da bibliografia o progetto (.bib/.zip)…":
    "From a bibliography or project (.bib/.zip)…",
  "Apri la guida":
    "Open the guide",
  "ⓘ recupera i metadati…":
    "ⓘ fetch the metadata…",
  "codice":
    "code",
  "Rilascia l'immagine per inserirla nell'appunto (o un PDF per importarlo)":
    "Drop the image to insert it into the note (or a PDF to import it)",
  "Rilascia i PDF per importarli":
    "Drop the PDFs to import them",
  "leggo le fonti {fatti}/{totale}":
    "reading the sources {fatti}/{totale}",
  "scrivo…":
    "writing…",
  "controllo le fonti…":
    "checking the sources…",
  "{concetto}: {fase}":
    "{concetto}: {fase}",
  "Hai copiato un link a un PDF":
    "You copied a link to a PDF",
  "Aggancia":
    "Grab it",
  "Riassunto AI: {fatti}/{totale}":
    "AI summary: {fatti}/{totale}",
  "Tag automatici AI: {fatti}/{totale}":
    "AI auto-tag: {fatti}/{totale}",
  "Import: {fatti}/{totale} — {file}":
    "Import: {fatti}/{totale} — {file}",
  "Metadati: {fatti}/{totale} — {aggiornati} aggiornati":
    "Metadata: {fatti}/{totale} — {aggiornati} updated",
  "Trova PDF: {fatti}/{totale} — {allegati} allegati":
    "Find PDF: {fatti}/{totale} — {allegati} attached",
  "copia":
    "copy",
  "Domanda su questo documento…":
    "Question about this document…",
  "Es. cosa dicono i miei paper su…":
    "E.g. what do my papers say about…",
  "Apri il PDF":
    "Open the PDF",
  "Fonte non utilizzata dalla sintesi — apri comunque":
    "Source not used in the synthesis — open it anyway",
  "Apri la cartella — {percorso}":
    "Open the folder — {percorso}",
  "Scrivi in Markdown…\n\nTrascina o incolla un'immagine per inserirla. Usa [[Titolo di un altro appunto]] per collegare un appunto, [[@citekey]] o [[Titolo del paper]] per un documento. $$ … $$ per una formula.":
    "Write in Markdown…\n\nDrag or paste an image to insert it. Use [[Title of another note]] to link a note, [[@citekey]] or [[Paper title]] for a document. $$ … $$ for a formula.",
  "Nascondi abstract":
    "Hide abstract",
  "Mostra abstract":
    "Show abstract",
  "Tag — {titolo}":
    "Tags — {titolo}",
  "Fatto":
    "Done",
  "Aggiungi a — {titolo}":
    "Add to — {titolo}",
  "Nessuna raccolta: creane una dalla barra laterale.":
    "No collections yet: create one from the sidebar.",
  "Copia Markdown":
    "Copy Markdown",
  "Le [n] diventano \\cite{citekey}":
    "The [n] markers become \\cite{citekey}",
  "Copia per LaTeX":
    "Copy for LaTeX",
  "Copia per Pandoc":
    "Copy for Pandoc",
  "📝 Salva negli Appunti":
    "📝 Save to Notes",
  "Salva .md…":
    "Save .md…",
  "Risultati raccolti dai paper":
    "Results harvested from the papers",
  "Valori estratti testualmente dai documenti selezionati (verifica sempre sul PDF: clic sul paper per aprirlo).":
    "Values quoted verbatim from the selected documents (always check them against the PDF: click a paper to open it).",
  "Per capire «{titolo}», in ordine consigliato: prima i fondamenti che cita, poi i vicini di contenuto già tuoi, infine i riferimenti che ancora non possiedi.":
    "To get to grips with «{titolo}», in the suggested order: first the foundations it cites, then the closest papers you already own, finally the references you don't have yet.",
  "questo paper":
    "this paper",
  "+ Aggiungi":
    "+ Add",
  "La pagina userà come fonti esattamente il documento selezionato — dovrà comparire nel testo, o essere dichiarato non pertinente.":
    "The page will use exactly the selected document as its source — it has to appear in the text, or be declared not relevant.",
  "La pagina userà come fonti esattamente i {n} documenti selezionati (i primi 10) — ognuno dovrà comparire nel testo, o essere dichiarato non pertinente.":
    "The page will use exactly the {n} selected documents as its sources (the first 10) — each one has to appear in the text, or be declared not relevant.",
  "La pagina userà come fonti esattamente i {n} documenti selezionati — ognuno dovrà comparire nel testo, o essere dichiarato non pertinente.":
    "The page will use exactly the {n} selected documents as its sources — each one has to appear in the text, or be declared not relevant.",
  "Se esiste già una pagina con lo stesso titolo verrà sostituita. La trovi poi in «Wiki della libreria».":
    "If a page with the same title already exists it will be replaced. You'll then find it under «Library wiki».",
  "è in libreria come <strong>citazione</strong>: non ha ancora un file allegato (quando è stato aggiunto non c'era un PDF Open Access scaricabile).":
    "is in your library as a <strong>citation</strong>: it has no file attached yet (there was no downloadable Open Access PDF when it was added).",
  "…oppure allega tu un link al PDF:":
    "…or attach a link to the PDF yourself:",
  "Il file viene scaricato e allegato a <strong>questa</strong> voce — tag, citazioni e metadati restano; nessun duplicato.":
    "The file is downloaded and attached to <strong>this</strong> entry — tags, citations and metadata stay put; no duplicate.",
  "Riferimento":
    "Reference",
  "Leggi ora":
    "Read now",
  "Aggiungi per identificatore":
    "Add by identifier",
  "Incolla DOI, ID arXiv, ISBN o PMID — uno per riga. Recupero i metadati e creo le voci (senza PDF allegato).":
    "Paste DOIs, arXiv IDs, ISBNs or PMIDs — one per line. I fetch the metadata and create the entries (with no PDF attached).",
  "Aggancia da URL":
    "Grab from a URL",
  "Incolla il link diretto a un PDF (deve terminare in <code>.pdf</code> o essere servito come PDF): lo scarico e lo aggiungo alla libreria, con i metadati se trovo un DOI. Per farlo con <strong>un clic dal browser</strong> usa il bookmarklet in":
    "Paste the direct link to a PDF (it has to end in <code>.pdf</code> or be served as a PDF): I download it and add it to the library, with metadata if I find a DOI. To do it with <strong>one click from the browser</strong> use the bookmarklet in",
  "Impostazioni → Connettore browser":
    "Settings → Browser connector",
  "Scarico…":
    "Downloading…",
  "versione {versione} · {anno}":
    "version {versione} · {anno}",
  "Gestore di PDF e riferimenti, locale e veloce. Tutto resta sul tuo computer; le funzioni di rete e AI sono opzionali e disattivabili.":
    "A local, fast manager for PDFs and references. Everything stays on your computer; the network and AI features are optional and can be switched off.",
  "Costruito con Tauri · Rust · SvelteKit · SQLite.":
    "Built with Tauri · Rust · SvelteKit · SQLite.",
  "Importa «{nome}»":
    "Import «{nome}»",
  "Dall'archivio verrà letta la <b>bibliografia</b> (i file <code>.bib</code>): ogni voce citata diventa una scheda in libreria.":
    "The <b>bibliography</b> will be read from the archive (the <code>.bib</code> files): every cited entry becomes a record in your library.",
  "Ogni voce del file diventa una scheda in libreria, con autori, anno, DOI e parole chiave come tag.":
    "Every entry in the file becomes a record in your library, with authors, year, DOI and keywords as tags.",
  "<b>Cerca i PDF open-access</b> delle voci senza file":
    "<b>Look for open-access PDFs</b> for the entries with no file",
  "arXiv, Unpaywall, OpenAlex, Semantic Scholar. Richiede la ricerca online attiva; su bibliografie lunghe richiede qualche minuto.":
    "arXiv, Unpaywall, OpenAlex, Semantic Scholar. Needs online search switched on; on long bibliographies it takes a few minutes.",
  "Raccogli tutto in «{nome}»":
    "Gather everything into «{nome}»",
  "Una raccolta dedicata, così le ritrovi insieme nell'Archivio. Alla fine ti ci porto.":
    "A collection of its own, so you find them together in the Archive. I'll take you there when it's done.",
  "Importa anche il progetto LaTeX completo":
    "Import the full LaTeX project too",
  "Estrae tutto l'archivio (.tex, immagini, classi) in un progetto compilabile in Progetti (LaTeX).":
    "Extracts the whole archive (.tex, images, class files) into a project you can compile under Projects (LaTeX).",
  "Cartella dei PDF esportati":
    "Folder of the exported PDFs",
  "Serve solo se l'esportazione tiene i PDF a parte (es. Zotero «Esporta file»).":
    "Only needed if the export keeps the PDFs separate (e.g. Zotero «Export files»).",
  "Scegli una cartella…":
    "Choose a folder…",
  "rimuovi":
    "remove",
  "Scriptorium {versione} è disponibile":
    "Scriptorium {versione} is available",
  "Hai la {versione}. La tua libreria, gli appunti e le impostazioni non vengono toccati.":
    "You have {versione}. Your library, notes and settings are left untouched.",
  "Finito lo scaricamento, <strong>Scriptorium si chiude</strong> per farsi sostituire e si riapre da solo. Chiudi prima quello che stai facendo altrove.":
    "Once the download finishes, <strong>Scriptorium closes</strong> so it can be replaced, then reopens on its own. Wrap up whatever you're doing elsewhere first.",
  "Scarico… {fatti} di {totale} MB":
    "Downloading… {fatti} of {totale} MB",
  "Scarico… {fatti} MB":
    "Downloading… {fatti} MB",
  "Poi parte l'installazione: una finestrella di avanzamento, nessuna domanda — e l'app si chiude da sola.":
    "Then the installer runs: a small progress window, no questions asked — and the app closes by itself.",
  "Installato ✓ — riavvia per usare la versione nuova.":
    "Installed ✓ — restart to use the new version.",
  "Più tardi":
    "Later",
  "Scarica e installa":
    "Download and install",
  "In corso…":
    "In progress…",
  "Riavvio io":
    "I'll restart it myself",
  "Riavvia ora":
    "Restart now",
  "Novità della {versione}":
    "What's new in {versione}",
  "Novità dalle ultime versioni":
    "What's new in the latest versions",
  "{versione} — {titolo}":
    "{versione} — {titolo}",
  "Carico i riferimenti…":
    "Loading the references…",
  "Citato nella tua libreria ({n})":
    "Cited in your library ({n})",
  "Nessun documento della libreria cita questo paper (servono i metadati/DOI dei tuoi documenti).":
    "No document in your library cites this paper (your documents need metadata/DOIs first).",
  "Bibliografia ({n})":
    "Bibliography ({n})",
  "Nessun riferimento estratto. Usa <strong>Metadati</strong> (in alto) per recuperarli da Crossref tramite il DOI.":
    "No references extracted. Use <strong>Metadata</strong> (at the top) to fetch them from Crossref via the DOI.",
  "{n} cit.":
    "{n} cit.",
  "Esplora ↗":
    "Explore ↗",
  "Esplora citazioni":
    "Explore citations",
  "← Indietro":
    "← Back",
  "{titolo} — da OpenAlex; clicca un nodo (o «Esplora ↗») per spostarti di paper in paper (snowball)":
    "{titolo} — from OpenAlex; click a node (or «Explore ↗») to move from paper to paper (snowballing)",
  "Carico la rete di citazioni…":
    "Loading the citation network…",
  "OpenAlex non riconosce questo paper — né per DOI né per titolo (per l'aggancio senza DOI serve un titolo che corrisponda esattamente). Recupera prima i metadati con «✦ senza metadati» o «Recupera metadati…».":
    "OpenAlex doesn't recognize this paper — neither by DOI nor by title (matching without a DOI needs a title that matches exactly). Fetch the metadata first with «✦ missing metadata» or «Fetch metadata…».",
  "Mappa":
    "Map",
  "{n} in libreria":
    "{n} in your library",
  "· nodo più grande = più citato":
    "· bigger node = more cited",
  "Riferimenti — cita ({n})":
    "References — cites ({n})",
  "⬇ Salva":
    "⬇ Save",
  "Nessun riferimento noto a OpenAlex per questo paper.":
    "OpenAlex knows of no references for this paper.",
  "Citato da ({n})":
    "Cited by ({n})",
  "Nessun paper che cita questo (ancora) su OpenAlex.":
    "No paper cites this one on OpenAlex (yet).",
  "Apri in libreria":
    "Open in the library",
  "+ Aggiungi alla libreria":
    "+ Add to the library",
  "Esplora da qui":
    "Explore from here",
  "Pagina del paper ↗":
    "Paper's page ↗",
  "Salute":
    "Health",
  "Analisi in corso…":
    "Analyzing…",
  "File mancanti sul disco":
    "Files missing from disk",
  "Il PDF non è più al percorso salvato: l'hai spostato o rinominato fuori da Scriptorium. «Ricollega…» ti fa indicare dov'è ora — appunti, evidenziazioni e tag restano; se hai spostato un'intera cartella, dopo il primo ti propongo di sistemare anche gli altri.":
    "The PDF is no longer at the saved path: you moved or renamed it outside Scriptorium. «Relink…» lets you point to where it is now — notes, highlights and tags are kept; if you moved a whole folder, after the first one I'll offer to fix the rest as well.",
  "PDF senza testo estratto":
    "PDFs with no extracted text",
  "Probabili scansioni (immagine): non cercabili né indicizzabili. «OCR» riconosce il testo con il motore di Windows.":
    "Probably scans (images): neither searchable nor indexable. «OCR» reads the text with the Windows engine.",
  "Metadati incompleti":
    "Incomplete metadata",
  "Manca titolo, anno o autori. «✦ senza metadati» in alto li recupera in blocco; «Trova…» cerca i candidati online per il singolo documento e scegli tu.":
    "Title, year or authors are missing. «✦ missing metadata» at the top fetches them in bulk; «Find…» searches online candidates for a single document and you pick.",
  "Senza incorporamento semantico":
    "No semantic embedding",
  "Esclusi dalla ricerca semantica e da «Correlati».":
    "Left out of semantic search and «Related».",
  "Senza copertina":
    "No cover",
  "Nessuna anteprima generata.":
    "No preview generated.",
  "1 documento analizzato.":
    "1 document analyzed.",
  "{n} documenti analizzati.":
    "{n} documents analyzed.",
  "{categoria} ({n})":
    "{categoria} ({n})",
  "Trova…":
    "Find…",
  "Ricollega…":
    "Relink…",
  "…e altri {n}.":
    "…and {n} more.",
  "Tutto a posto ✓":
    "All clear ✓",
  "Duplicati — stesso file ({n})":
    "Duplicates — same file ({n})",
  "{titolo} — 1 copia":
    "{titolo} — 1 copy",
  "{titolo} — {n} copie":
    "{titolo} — {n} copies",
  "Unisci i duplicati dalla scheda <strong>Duplicati</strong> qui sopra.":
    "Merge the duplicates from the <strong>Duplicates</strong> tab above.",
  "Nessun duplicato ✓":
    "No duplicates ✓",
  "I DOI che la tua libreria cita di più ma che non possiedi ancora. Si basa sui riferimenti estratti — recupera i <strong>Metadati</strong> dei tuoi paper (Crossref) per arricchirli.":
    "The DOIs your library cites most but doesn't own yet. It works off the extracted references — fetch your papers' <strong>Metadata</strong> (Crossref) to enrich them.",
  "Interrompi":
    "Stop",
  "Risolvo {fatti}/{totale} — {trovati} DOI trovati…":
    "Resolving {fatti}/{totale} — {trovati} DOIs found…",
  "Risolvi DOI dei riferimenti (online)":
    "Resolve reference DOIs (online)",
  "Recupera i DOI mancanti dei riferimenti già in libreria — precision-first, nessun abbinamento incerto.":
    "Fetches the missing DOIs of the references already in your library — precision-first, no uncertain matches.",
  "Calcolo in corso…":
    "Computing…",
  "Nessun gap rilevato. Servono riferimenti con DOI: apri un documento → <strong>Riferimenti e citazioni</strong>, oppure recupera i <strong>Metadati</strong> da Crossref.":
    "No gaps found. You need references with DOIs: open a document → <strong>References and citations</strong>, or fetch the <strong>Metadata</strong> from Crossref.",
  "Copie dello stesso lavoro (per DOI o titolo+anno). «Unisci» tiene la prima e vi trasferisce tag, raccolte e annotazioni; le altre finiscono nel cestino.":
    "Copies of the same work (by DOI or title+year). «Merge» keeps the first one and moves tags, collections and annotations onto it; the others go to the trash.",
  "Codice & repository":
    "Code & repositories",
  "Repository GitHub ({n})":
    "GitHub repositories ({n})",
  "Cerco su GitHub…":
    "Searching GitHub…",
  "Nessun repository GitHub trovato nel testo di questo documento.":
    "No GitHub repository found in the text of this document.",
  "Apri su GitHub":
    "Open on GitHub",
  "README · {repo}":
    "README · {repo}",
  "Carico il README…":
    "Loading the README…",
  "Cerco su Hugging Face…":
    "Searching Hugging Face…",
  "Nessun identificatore arXiv per questo documento: non posso collegarlo a modelli/dataset su Hugging Face.":
    "No arXiv identifier for this document: I can't link it to models or datasets on Hugging Face.",
  "Apri la pagina del paper su Hugging Face ↗":
    "Open the paper's page on Hugging Face ↗",
  "Modelli ({n})":
    "Models ({n})",
  "Nessun modello collegato.":
    "No linked models.",
  "Dataset ({n})":
    "Datasets ({n})",
  "Nessun dataset collegato.":
    "No linked datasets.",
  "Ricerca online":
    "Online search",
  "Connettore browser":
    "Browser connector",
  "CLI e MCP":
    "CLI and MCP",
  "Backup":
    "Backup",
  "Manutenzione":
    "Maintenance",
  "La ricerca online è una funzione di rete: finché è disattivata, l'app resta 100% offline. I PDF vengono scaricati solo per i lavori Open Access.":
    "Online search is a network feature: while it is off, the app stays 100% offline. PDFs are downloaded only for Open Access works.",
  "Abilita funzioni online (ricerca su arXiv, OpenAlex, ADS, Semantic Scholar e altre fonti; Trova PDF, esplorazione citazioni, recupero metadati)":
    "Enable online features (search on arXiv, OpenAlex, ADS, Semantic Scholar and other sources; Find PDF, citation exploration, metadata retrieval)",
  "Email di contatto — opzionale ma consigliata":
    "Contact email — optional but recommended",
  "Inviata agli archivi (Crossref, OpenAlex, Unpaywall) per identificarti gentilmente: dà limiti di richiesta più alti e meno blocchi. È <strong>richiesta</strong> per “Trova PDF” (Unpaywall). Non viene usata per altro.":
    "Sent to the archives (Crossref, OpenAlex, Unpaywall) to identify you politely: it buys higher rate limits and fewer blocks. It is <strong>required</strong> for “Find PDF” (Unpaywall). It is not used for anything else.",
  "🔒 Le chiavi sono salvate nel <strong>Credential Manager di Windows</strong> (cifrate), non nel database. Dopo il salvataggio non sono più visibili: puoi solo sostituirle o rimuoverle.":
    "🔒 Keys are stored in the <strong>Windows Credential Manager</strong> (encrypted), not in the database. Once saved they are no longer visible: you can only replace or remove them.",
  "Chiave API OpenAlex":
    "OpenAlex API key",
  "Gratuita su openalex.org/settings/api (opzionale: il free tier funziona anche senza).":
    "Free at openalex.org/settings/api (optional: the free tier works without one).",
  "Token API ADS":
    "ADS API token",
  "Gratuito su ui.adsabs.harvard.edu (Account → API Token). Richiesto per la fonte ADS.":
    "Free at ui.adsabs.harvard.edu (Account → API Token). Required for the ADS source.",
  "Chiave API Semantic Scholar":
    "Semantic Scholar API key",
  "Opzionale (alza i limiti), su semanticscholar.org/product/api.":
    "Optional (raises the limits), at semanticscholar.org/product/api.",
  "Chiave API CORE":
    "CORE API key",
  "Gratuita su core.ac.uk/services/api. Richiesta per la fonte CORE.":
    "Free at core.ac.uk/services/api. Required for the CORE source.",
  "Token GitHub":
    "GitHub token",
  "Opzionale (alza il limite di richieste per README/repo), su github.com/settings/tokens.":
    "Optional (raises the request limit for READMEs and repos), at github.com/settings/tokens.",
  "Locali e disattivate di default. Richiedono <strong>Ollama</strong> oppure <strong>LM Studio</strong> installato, con almeno un modello caricato.":
    "Local, and off by default. They need <strong>Ollama</strong> or <strong>LM Studio</strong> installed, with at least one model loaded.",
  "Puoi <em>Avviare</em>/<em>Fermare</em> il server direttamente da qui (richiede Ollama nel PATH, o la CLI <code>lms</code> di LM Studio), premere <em>Verifica</em> per vedere i modelli, poi scegliere quale usare nel menu in fondo. Quando un provider è raggiungibile compare l'indicatore <strong>AI</strong> in alto.":
    "You can <em>Start</em>/<em>Stop</em> the server right here (needs Ollama on the PATH, or LM Studio's <code>lms</code> CLI), press <em>Check</em> to list the models, then pick which one to use in the menu below. When a provider is reachable the <strong>AI</strong> indicator appears at the top.",
  "Abilita le funzioni AI":
    "Enable the AI features",
  "Ollama — URL del server":
    "Ollama — server URL",
  "Verifica":
    "Check",
  "Avvia":
    "Start",
  "Ferma":
    "Stop",
  "✓ raggiungibile — 1 modello":
    "✓ reachable — 1 model",
  "✓ raggiungibile — {n} modelli":
    "✓ reachable — {n} models",
  "✓ raggiungibile, ma nessun modello (scaricane uno: ollama pull …)":
    "✓ reachable, but no models (pull one: ollama pull …)",
  "✗ non raggiungibile — avvia Ollama":
    "✗ not reachable — start Ollama",
  "LM Studio — URL del server":
    "LM Studio — server URL",
  "✓ raggiungibile, ma nessun modello caricato in LM Studio":
    "✓ reachable, but no model loaded in LM Studio",
  "✗ non raggiungibile — avvia il server di LM Studio":
    "✗ not reachable — start the LM Studio server",
  "Modello da usare (scelto tra quelli trovati)":
    "Model to use (picked from the ones found)",
  "{modello} — attuale ({provider})":
    "{modello} — current ({provider})",
  "Avvia un provider e premi «Verifica» per vedere i modelli":
    "Start a provider and press «Check» to list the models",
  "Indicizzazione su GPU (via Ollama)":
    "Index on the GPU (via Ollama)",
  "Calcola gli embeddings dell'indice con la GPU tramite Ollama (modello <code>bge-m3</code>, 1024-dim, compatibile con l'indice esistente) invece del modello CPU integrato. Richiede Ollama avviato e <code>ollama pull bge-m3</code>.":
    "Computes the index embeddings on the GPU through Ollama (<code>bge-m3</code> model, 1024-dim, compatible with the existing index) instead of the built-in CPU model. Needs Ollama running and <code>ollama pull bge-m3</code>.",
  "Su 6 GB di VRAM condivide la memoria con l'LLM: conviene indicizzare con l'LLM scarico. Se cambi metodo, <strong>Ricostruisci</strong> l'indice in «Chiedi alla libreria» per coerenza.":
    "On 6 GB of VRAM it shares memory with the LLM: better to index with the LLM unloaded. If you switch method, <strong>Rebuild</strong> the index in «Ask your library» to keep it consistent.",
  "Dimensione batch embeddings — 0 = automatico (64 su GPU, 16 su CPU)":
    "Embedding batch size — 0 = automatic (64 on GPU, 16 on CPU)",
  "Su GPU potenti (es. RTX 4090/5090) alza a <strong>128–256</strong> per saturare la GPU e velocizzare l'indicizzazione. Su CPU lascia basso (8–16).":
    "On powerful GPUs (e.g. RTX 4090/5090) raise it to <strong>128–256</strong> to saturate the GPU and speed indexing up. On CPU keep it low (8–16).",
  "Esporta i tuoi documenti come note <strong>Markdown</strong> in un vault <strong>Obsidian</strong> (funziona anche con Logseq, Zettlr, Foam…).":
    "Export your documents as <strong>Markdown</strong> notes into an <strong>Obsidian</strong> vault (works with Logseq, Zettlr, Foam… too).",
  "Ogni documento diventa una nota <code>.md</code> in <code>&lt;vault&gt;/Scriptorium/</code> con metadati, abstract, note, annotazioni e tag/autori come <code>[[wikilink]]</code> per il grafo.":
    "Every document becomes an <code>.md</code> note under <code>&lt;vault&gt;/Scriptorium/</code> with metadata, abstract, notes, annotations, and tags and authors as <code>[[wikilink]]</code> entries for the graph.",
  "L'esportazione è a senso unico (Scriptorium → vault) e sovrascrive le note esistenti con lo stesso titolo.":
    "The export is one-way (Scriptorium → vault) and overwrites existing notes with the same title.",
  "Cartella del vault":
    "Vault folder",
  "Scegli…":
    "Choose…",
  "Per esportare usa il pulsante <strong>→ Obsidian</strong> in alto: invia i documenti mostrati (o quelli selezionati).":
    "To export use the <strong>→ Obsidian</strong> button at the top: it sends the documents on screen (or the selected ones).",
  "Aggancia un PDF direttamente dal browser con <strong>un clic</strong>. Trascina una volta il pulsante qui sotto nella <strong>barra dei preferiti</strong>; poi, quando sei su un PDF (o su una pagina che ne contiene uno), cliccalo e il file finisce nella tua libreria. Perfetto con arXiv e i PDF ad accesso aperto.":
    "Grab a PDF straight from the browser with <strong>one click</strong>. Drag the button below into your <strong>bookmarks bar</strong> once; then, whenever you are on a PDF (or on a page containing one), click it and the file lands in your library. Perfect with arXiv and open-access PDFs.",
  "Abilita il connettore (server locale su 127.0.0.1) — disattivato di default":
    "Enable the connector (local server on 127.0.0.1) — off by default",
  "Disattivato — attiva l'interruttore per usare il bookmarklet.":
    "Off — flip the switch to use the bookmarklet.",
  "✓ In ascolto sulla porta <code>{porta}</code>.":
    "✓ Listening on port <code>{porta}</code>.",
  "⚠ Attivo, ma non ho trovato una porta libera: chiudi eventuali conflitti e riattiva.":
    "⚠ On, but I could not find a free port: close whatever is conflicting and switch it on again.",
  "Trascina nella barra dei preferiti":
    "Drag into the bookmarks bar",
  "Trascina il pulsante nella barra dei preferiti (non cliccarlo qui)":
    "Drag the button into the bookmarks bar (do not click it here)",
  "Copia bookmarklet":
    "Copy bookmarklet",
  "Non cliccarlo qui: <strong>trascinalo</strong> nella barra dei preferiti. In alternativa premi «Copia bookmarklet», crea un nuovo preferito e incolla il testo come <em>indirizzo/URL</em>.":
    "Do not click it here: <strong>drag it</strong> into the bookmarks bar. Alternatively press «Copy bookmarklet», create a new bookmark and paste the text as its <em>address/URL</em>.",
  "🔒 Il server ascolta solo in locale (<code>127.0.0.1</code>, non raggiungibile dalla rete) ed è protetto da un <strong>token segreto</strong> incluso solo nel tuo bookmarklet: nessun altro sito può aggiungere PDF. Il download passa dagli stessi controlli anti-abuso del resto dell'app (solo https, solo file PDF). Se disattivi e riattivi il connettore, o cambia la porta, ri-trascina il bookmarklet aggiornato.":
    "🔒 The server listens locally only (<code>127.0.0.1</code>, unreachable from the network) and is protected by a <strong>secret token</strong> embedded in your bookmarklet alone: no other site can add PDFs. Downloads go through the same anti-abuse checks as the rest of the app (https only, PDF files only). If you switch the connector off and on again, or the port changes, drag the updated bookmarklet across once more.",
  "Sui siti che bloccano le richieste dirette (es. <strong>GitHub</strong>), il bookmarklet apre una piccola scheda di conferma di Scriptorium che completa l'aggancio. I link ai PDF nelle pagine GitHub (<code>…/blob/…</code>) vengono riscritti automaticamente verso il file vero.":
    "On sites that block direct requests (e.g. <strong>GitHub</strong>), the bookmarklet opens a small Scriptorium confirmation tab that finishes the job. Links to PDFs on GitHub pages (<code>…/blob/…</code>) are rewritten to the real file automatically.",
  "Appunti intelligenti":
    "Smart clipboard",
  "Suggerisci l'aggancio dei link PDF copiati":
    "Offer to grab copied PDF links",
  "Il metodo più semplice: <strong>copia il link</strong> del PDF nel browser e torna su Scriptorium — comparirà il suggerimento in basso a destra. Il testo copiato viene letto solo quando l'app torna in primo piano e non lasciano mai il tuo computer; non parte nulla finché non clicchi «Aggancia».":
    "The simplest route: <strong>copy the link</strong> to the PDF in the browser and come back to Scriptorium — the prompt appears at the bottom right. The copied text is read only when the app returns to the foreground and never leaves your computer; nothing happens until you click «Grab».",
  "Due compagni <strong>in sola lettura</strong> per usare la libreria da fuori (sicuri anche con l'app aperta): la <strong>CLI</strong> per il terminale e il <strong>server MCP</strong> per Claude Desktop / Claude Code e qualsiasi client MCP. Nessun servizio resta in ascolto: è il client ad avviare il processo quando serve e a chiuderlo a fine sessione.":
    "Two <strong>read-only</strong> companions for using your library from outside (safe even with the app open): the <strong>CLI</strong> for the terminal and the <strong>MCP server</strong> for Claude Desktop / Claude Code and any MCP client. No service stays listening: the client starts the process when it needs it and shuts it down at the end of the session.",
  "Server MCP — registralo in Claude Code con questo comando":
    "MCP server — register it in Claude Code with this command",
  "Comando copiato ✓":
    "Command copied ✓",
  "✓ binario presente accanto all'app":
    "✓ binary present next to the app",
  "⚠ binario non trovato — scarica scriptorium-mcp.exe dalle Release e mettilo accanto all'app":
    "⚠ binary not found — download scriptorium-mcp.exe from the Releases and put it next to the app",
  "— 9 strumenti: ricerca libreria, schede, BibTeX, appunti, ricerca appunti, progetti LaTeX, statistiche. Solo lettura.":
    "— 9 tools: library search, records, BibTeX, notes, note search, LaTeX projects, stats. Read-only.",
  "Per Claude Desktop: aggiungi questa voce in <code>claude_desktop_config.json</code> →":
    "For Claude Desktop: add this entry to <code>claude_desktop_config.json</code> →",
  "Config copiata ✓":
    "Config copied ✓",
  "CLI da terminale (query, list, show, bib, notes, note, search-notes, projects, stats)":
    "Terminal CLI (query, list, show, bib, notes, note, search-notes, projects, stats)",
  "⚠ binario non trovato — scarica scriptorium-cli.exe dalle Release":
    "⚠ binary not found — download scriptorium-cli.exe from the Releases",
  "— <code>scriptorium-cli help</code> per tutti i comandi; output JSON, comodo per script e Claude Code.":
    "— <code>scriptorium-cli help</code> for the full command list; JSON output, handy for scripts and for Claude Code.",
  "Carico i percorsi…":
    "Loading the paths…",
  "Salva una copia completa (database + PDF + miniature) in una cartella a tua scelta.":
    "Saves a complete copy (database + PDFs + thumbnails) to a folder of your choice.",
  "Scegli cartella e salva backup…":
    "Choose a folder and save a backup…",
  "<strong>Ripristina</strong> la libreria da un backup precedente. <strong>Sostituisce</strong> i dati attuali (ne salva prima una copia di sicurezza) e riavvia l'app. I dati non vengono mai persi installando o disinstallando: vivono in <code>{percorso}</code>, separati dal programma.":
    "<strong>Restore</strong> the library from an earlier backup. It <strong>replaces</strong> the current data (saving a safety copy of it first) and restarts the app. Installing or uninstalling never loses your data: it lives in <code>{percorso}</code>, separate from the program.",
  "Ripristina da cartella di backup…":
    "Restore from a backup folder…",
  "…o da un file .db (backup automatici in <code>{cartella}</code>)":
    "…or from a .db file (automatic backups in <code>{cartella}</code>)",
  "<strong>Verifica e ripara metadati.</strong> Controlla ogni documento e corregge quelli il cui titolo non corrisponde al PDF — di solito perché l'arricchimento ha pescato il DOI di un <em>lavoro citato</em> invece di quello del documento. I paper <strong>arXiv</strong> recuperano i dati corretti da arXiv (anche quelli ancora senza metadati); per gli altri il titolo viene ricavato dalla prima riga del PDF. I documenti già corretti non vengono toccati. È sicuro e ripetibile; può richiedere fino a un minuto.":
    "<strong>Check and repair metadata.</strong> Goes through every document and fixes the ones whose title does not match the PDF — usually because enrichment picked up the DOI of a <em>cited work</em> instead of the document's own. <strong>arXiv</strong> papers get the correct data back from arXiv (including those still without metadata); for the rest the title is taken from the first line of the PDF. Documents that are already correct are left alone. It is safe and repeatable; it can take up to a minute.",
  "Riparazione in corso…":
    "Repairing…",
  "Verifica e ripara metadati":
    "Check and repair metadata",
  "<strong>Plancia — registro su file.</strong> La Plancia (icona tachimetro) mostra in tempo reale i processi interni; qui puoi far scrivere lo stesso registro anche su file, uno al giorno (conservati gli ultimi 14), utile per capire a posteriori cosa è successo.":
    "<strong>Bridge — log to file.</strong> The Bridge (speedometer icon) shows the internal processes in real time; here you can have the same log written to a file as well, one per day (the last 14 are kept), useful for working out after the fact what happened.",
  "Registra l'attività della Plancia su file":
    "Log Bridge activity to a file",
  "I file sono in":
    "The files are in",
  "apri cartella":
    "open the folder",
  "Selezione ({n})":
    "Selection ({n})",
  "Documento":
    "Document",
  "Vai a":
    "Go to",
  "Tutti":
    "All",
  "tutta la libreria":
    "the whole library",
  "Preferiti":
    "Favorites",
  "Da leggere":
    "Unread",
  "Con codice (GitHub)":
    "With code (GitHub)",
  "Peer-reviewed":
    "Peer-reviewed",
  "Il mio lavoro":
    "My own work",
  "Cestino":
    "Trash",
  "Raccolta: {nome}":
    "Collection: {nome}",
  "Tag: {nome}":
    "Tag: {nome}",
  "Ricerca salvata: {nome}":
    "Saved search: {nome}",
  "rilancia e mostra le novità":
    "runs it again and shows what's new",
  "Appunto: {titolo}":
    "Note: {titolo}",
  "Appunti":
    "Notes",
  "appunto nota md":
    "notes memo md markdown",
  "Wiki: {titolo}":
    "Wiki: {titolo}",
  "Wiki":
    "Wiki",
  "wiki pagina concetto":
    "wiki page concept entry topic",
  "Progetto LaTeX: {nome}":
    "LaTeX project: {nome}",
  "Progetti":
    "Projects",
  "latex progetto tex overleaf":
    "latex project tex overleaf writing paper",
  "Inizia qui":
    "Start here",
  "Libreria":
    "Library",
  "Lettura":
    "Reading",
  "Scrittura":
    "Writing",
  "Scoperta":
    "Discovery",
  "AI & dati":
    "AI & data",
  "FAQ — Come faccio a…":
    "FAQ — How do I…",
  "Guida: {scheda}":
    "Guide: {scheda}",
  "aiuto help manuale documentazione faq":
    "help manual documentation guide faq how-to",
  "Tema: {nome}":
    "Theme: {nome}",
  "scuro":
    "dark",
  "chiaro":
    "light",
  "Aspetto":
    "Appearance",
  "tema aspetto colori":
    "theme appearance colours colors palette look",
  "Documenti":
    "Documents",
  "Carta":
    "Paper",
  "Seppia":
    "Sepia",
  "Solarized":
    "Solarized",
  "Salvia":
    "Sage",
  "Pastello":
    "Pastel",
  "Medievale":
    "Medieval",
  "Scuro":
    "Dark",
  "Nord":
    "Nord",
  "Grafite":
    "Graphite",
  "Foresta":
    "Forest",
  "Synthwave":
    "Synthwave",
  "Primo autore":
    "First author",
  "Rivista":
    "Journal",
  "Aggiunto":
    "Added",
  "Ordina":
    "Sort",
  "Ordina: {criterio} {freccia}":
    "Sort: {criterio} {freccia}",
  "Ordina: {criterio} {freccia} +{altri}":
    "Sort: {criterio} {freccia} +{altri}",
  "Ordina per {criterio}":
    "Sort by {criterio}",
  "Tutto":
    "All",
  "Semantica":
    "Semantic",
  "Combina ricerca testuale e semantica e fonde i risultati (consigliato)":
    "Combines full-text and semantic search and merges the results (recommended)",
  "Cerca le parole esatte nel testo dei PDF (veloce, non richiede l'indice)":
    "Looks for the exact words in the text of your PDFs (fast, no index needed)",
  "Trova per significato, anche con parole diverse (richiede l'indice semantico)":
    "Finds by meaning, even when the words differ (needs the semantic index)",
  "Copia il PDF e apre WhatsApp: incolla con Ctrl+V":
    "Copies the PDF and opens WhatsApp: paste it with Ctrl+V",
  "Copia il PDF e apre Teams":
    "Copies the PDF and opens Teams",
  "Copia il PDF e apre Gmail":
    "Copies the PDF and opens Gmail",
  "Outlook desktop: PDF allegato direttamente":
    "Outlook desktop: the PDF is attached directly",
  "Mostra nella cartella":
    "Show in folder",
  "Copia percorso":
    "Copy path",
  "Copia il percorso del file PDF":
    "Copies the full path of the PDF file",
  "Copia APA":
    "Copy APA",
  "Copia IEEE":
    "Copy IEEE",
  "Copia BibTeX":
    "Copy BibTeX",
  "Copia citekey":
    "Copy citekey",
  "Copia \\cite{…}":
    "Copy \\cite{…}",
  "Pronto per LaTeX":
    "Ready to paste into LaTeX",
  "Copia [@…]":
    "Copy [@…]",
  "Pronto per Pandoc/Quarto":
    "Ready to paste into Pandoc/Quarto",
  "Riferimenti e citazioni":
    "References and citations",
  "Bibliografia del paper e chi lo cita nella tua libreria":
    "The paper's bibliography, and which documents in your library cite it",
  "Esplora citazioni (online)":
    "Explore citations (online)",
  "Snowball su OpenAlex: citazioni da e verso questo paper, aggiungile alla libreria":
    "Snowball search on OpenAlex: what this paper cites and what cites it, ready to add to your library",
  "Snowball su OpenAlex — senza DOI il paper si aggancia per titolo (corrispondenza rigorosa)":
    "Snowball search on OpenAlex — with no DOI the paper is matched by title (strict matching)",
  "Riassunto ✓ (rigenera)":
    "Summary ✓ (regenerate)",
  "Riassumi":
    "Summarize",
  "Già presente: cliccando lo rigeneri e sovrascrivi":
    "Already there: clicking regenerates it and overwrites the old one",
  "Riassunto in italiano con l'AI locale":
    "A summary written by the local AI, in the language set for AI answers",
  "Tag automatici (ha già tag)":
    "Auto-tag (already has tags)",
  "Tag automatici":
    "Auto-tag",
  "Ha già {n} tag: la chiamata può aggiungerne altri":
    "Already has {n} tags: this run can add more",
  "Suggerisce e assegna tag tematici":
    "Suggests and applies topical tags",
  "Chiedi al documento":
    "Ask this document",
  "Domande in linguaggio naturale su questo PDF":
    "Natural-language questions about this PDF",
  "Correlati":
    "Related",
  "I documenti più vicini per significato (indice semantico)":
    "The documents closest in meaning (semantic index)",
  "documento":
    "document",
  "Cosa leggere prima per capirlo: fondamenti citati + vicini precedenti (senza LLM)":
    "What to read first to understand it: the foundations it cites + earlier neighbors (no LLM involved)",
  "Recupera metadati…":
    "Fetch metadata…",
  "Cerca online la scheda giusta (Crossref, arXiv, OpenAlex) e scegli tu quale applicare":
    "Searches online for the right record (Crossref, arXiv, OpenAlex) and lets you choose which one to apply",
  "Tag…":
    "Tags…",
  "Assegna o togli tag":
    "Add or remove tags",
  "Raccolte…":
    "Collections…",
  "Aggiungi a una raccolta":
    "Add to a collection",
  "Togli da «{dove}»":
    "Remove from “{dove}”",
  "raccolta":
    "collection",
  "Questa voce è solo un riferimento: trova un PDF Open Access o allegane uno da un link":
    "This entry is a reference only: find an Open Access PDF or attach one from a link",
  "Leggi nel visore integrato":
    "Read it in the built-in viewer",
  "Copia titolo":
    "Copy title",
  "Copia il titolo del paper":
    "Copies the paper's title",
  "Copia citazioni, riferimenti, esplora":
    "Copy citations, browse references, explore online",
  "Riassunto, tag, domande, correlati":
    "Summary, tags, questions, related documents",
  "Correlati (per il resto attiva l'AI locale)":
    "Related documents (turn the local AI on for the rest)",
  "Organizza":
    "Organize",
  "Metadati, tag, raccolte":
    "Metadata, tags, collections",
  "Codice & repo":
    "Code & repos",
  "Repository GitHub citati + modelli e dataset Hugging Face":
    "GitHub repositories cited in the paper + Hugging Face models and datasets",
  "Invia, stampa, mostra file":
    "Send, print, show the file",
  "Sposta nel cestino (recuperabile)":
    "Moves it to the trash (you can get it back)",
  "Trova PDF…":
    "Find PDF…",
  "Mostra i candidati trovati online (arXiv, OpenAlex, Semantic Scholar, Crossref, per identificativo e per titolo): scegli tu quale scaricare e allegare":
    "Shows the candidates found online (arXiv, OpenAlex, Semantic Scholar, Crossref, by identifier and by title): you choose which one to download and attach",
  "{n} documenti PDF":
    "{n} PDF documents",
  "Un unico lavoro di stampa":
    "A single print job for all of them",
  "Trova PDF (1 riferimento)":
    "Find PDF (1 reference)",
  "Trova PDF ({n} riferimenti)":
    "Find PDFs ({n} references)",
  "Cerca e allega la copia Open Access per le voci selezionate senza file (arXiv, Unpaywall, OpenAlex, Semantic Scholar)":
    "Searches for and attaches the Open Access copy of the selected entries that have no file (arXiv, Unpaywall, OpenAlex, Semantic Scholar)",
  "Riassumi (AI)":
    "Summarize (AI)",
  "Un riassunto per ogni selezionato":
    "One summary for each selected document",
  "Tag automatici (AI)":
    "Auto-tag (AI)",
  "Confronta (AI)":
    "Compare (AI)",
  "Tabella: obiettivo, metodo, dati, risultati, limiti — e cosa aggiunge ciascuno":
    "A table: goal, method, data, results, limits — and what each one adds",
  "Rassegna (AI)":
    "Review (AI)",
  "Mini related-work per temi (2-10 paper); salvabile come appunto con backlink [[@citekey]]":
    "A short related-work section by theme (2-10 papers); can be saved as a note with [[@citekey]] backlinks",
  "Tabella risultati (AI)":
    "Results table (AI)",
  "Raccogli metriche e numeri dei paper in un'unica tabella (CSV/Excel)":
    "Harvests the metrics and numbers from the papers into one table (CSV/Excel)",
  "Pagina wiki (AI)":
    "Wiki page (AI)",
  "Una pagina della Wiki con esattamente questi documenti come fonti (max 10)":
    "A wiki page using exactly these documents as its sources (max 10)",
  "In raccolta":
    "Add to collection",
  "Copia le citazioni della selezione per LaTeX/Pandoc":
    "Copy the citations of the whole selection for LaTeX/Pandoc",
  "Un solo \\cite con tutte le chiavi":
    "A single \\cite holding every key",
  "Le voci .bib di tutti i selezionati":
    "The .bib entries of every selected document",
  "Copia [@…] (Pandoc)":
    "Copy [@…] (Pandoc)",
  "Esporta citazioni":
    "Export citations",
  "Salva un file .bib/.ris/.json":
    "Saves a .bib/.ris/.json file",
  "I miei paper":
    "My papers",
  "Torna alla griglia dei paper (tutta la libreria)":
    "Back to the paper grid (the whole library)",
  "Importa":
    "Import",
  "PDF, BibTeX, identificatori, URL":
    "PDFs, BibTeX, identifiers, URLs",
  "PDF dal disco…":
    "PDFs from disk…",
  "Da bibliografia o progetto…":
    "From a bibliography or a project…",
  "Zotero/Mendeley/EndNote (.bib/.ris/CSL-JSON) o uno .zip Overleaf: crea le schede, scarica i PDF open-access, tag e raccolta":
    "Zotero/Mendeley/EndNote (.bib/.ris/CSL-JSON) or an Overleaf .zip: creates the records, downloads the open-access PDFs, applies tags and a collection",
  "Progetto LaTeX (.zip)…":
    "LaTeX project (.zip)…",
  "I tuoi paper: PDF + bibliografia":
    "Your own papers: PDF + bibliography",
  "Per identificatore…":
    "By identifier…",
  "Da URL…":
    "From a URL…",
  "Scarica un PDF da un link":
    "Downloads a PDF from a link",
  "Cartella sorvegliata…":
    "Watched folder…",
  "Importa automaticamente i PDF che aggiungi":
    "Imports automatically every PDF you drop in",
  "Griglia, lista, costellazione, ordinamento":
    "Grid, list, constellation, sorting",
  "Lista":
    "List",
  "Costellazione della raccolta":
    "Constellation of this collection",
  "Costellazione":
    "Constellation",
  "La mappa dei soli paper di questa raccolta (dalla barra in alto torni a tutta la libreria)":
    "The map of this collection's papers only (the top bar takes you back to the whole library)",
  "Mappa semantica della libreria":
    "Semantic map of the library",
  "Costellazione di tutta la libreria":
    "Constellation of the whole library",
  "La mappa completa, ignorando il filtro attivo":
    "The complete map, ignoring the active filter",
  "Mostra/nascondi (Ctrl+B)":
    "Show/hide (Ctrl+B)",
  "Ordina: {criterio}":
    "Sort: {criterio}",
  "Un tocco attiva, un altro inverte, un terzo toglie":
    "One click turns it on, a second reverses it, a third removes it",
  "Riprendi lettura":
    "Resume reading",
  "Torna all'ultimo PDF, al punto in cui eri":
    "Back to the last PDF, at the page you left off",
  "Chiedi alla libreria":
    "Ask your library",
  "Risposte con citazioni dai tuoi PDF (AI locale)":
    "Answers with citations drawn from your own PDFs (local AI)",
  "Wiki della libreria":
    "Library wiki",
  "La tua enciclopedia privata, generata dai tuoi paper":
    "Your private encyclopedia, written from your own papers",
  "Cerca online":
    "Search online",
  "arXiv, OpenAlex, ADS e altre fonti":
    "arXiv, OpenAlex, ADS and other sources",
  "I tuoi appunti in Markdown (file .md) con [[collegamenti]]":
    "Your notes in Markdown (.md files) with [[links]]",
  "Progetti (LaTeX)":
    "Projects (LaTeX)",
  "Scrivi in LaTeX con citazioni dalla libreria; compila con Tectonic/latexmk":
    "Write in LaTeX with citations from your library; build with Tectonic/latexmk",
  "Archivio":
    "Archive",
  "Raccolte e sotto-raccolte in vista sinottica: organizza i paper trascinandoli":
    "Collections and sub-collections at a glance: organize papers by dragging them",
  "Un documento dimenticato, pescato per te":
    "A forgotten document, picked out for you",
  "Novità":
    "New papers",
  "Nuovi paper sui temi che segui (ricerche salvate), raccolti a ogni avvio":
    "New papers on the topics you follow (saved searches), collected at every launch",
  "Esporta":
    "Export",
  "Citazioni (BibTeX/RIS/CSL)…":
    "Citations (BibTeX/RIS/CSL)…",
  "In Obsidian (Markdown)":
    "To Obsidian (Markdown)",
  "Cura della libreria":
    "Library care",
  "Salute, gap di citazioni, duplicati e manutenzione":
    "Health, citation gaps, duplicates and maintenance",
  "Salute libreria":
    "Library health",
  "File mancanti, PDF senza testo, metadati incompleti…":
    "Missing files, PDFs with no text, incomplete metadata…",
  "Recupera metadati mancanti":
    "Fetch missing metadata",
  "1 documento incompleto — arXiv dal nome file, DOI e titolo dal PDF":
    "1 incomplete document — arXiv from the file name, DOI and title from the PDF",
  "{n} documenti incompleti — arXiv dal nome file, DOI e titolo dal PDF":
    "{n} incomplete documents — arXiv from the file name, DOI and title from the PDF",
  "Nessun documento incompleto al momento":
    "No incomplete documents right now",
  "Trova PDF dei riferimenti":
    "Find PDFs for the references",
  "Cerca copie Open Access (arXiv, Unpaywall, OpenAlex, Semantic Scholar) per TUTTE le voci senza file e le allega":
    "Searches for Open Access copies (arXiv, Unpaywall, OpenAlex, Semantic Scholar) for EVERY entry with no file, and attaches them",
  "Gap di citazioni":
    "Citation gaps",
  "I DOI più citati dai tuoi paper che ancora non possiedi":
    "The DOIs your papers cite most that you don't own yet",
  "Duplicati":
    "Duplicates",
  "Trova e unisci le copie dello stesso lavoro":
    "Finds and merges copies of the same work",
  "Rigenera anteprime":
    "Rebuild previews",
  "Ricrea le copertine dal PDF ad alta risoluzione":
    "Re-renders the covers from the PDF at high resolution",
  "{fatti}/{totale} indicizzati — abilita ricerca per significato, Correlati e Costellazione":
    "{fatti}/{totale} indexed — powers search by meaning, Related and the Constellation",
  "Memoria AI":
    "AI memory",
  "Libera la GPU (scarica i modelli) o ferma del tutto l'AI locale":
    "Free the GPU (unload the models) or shut the local AI down completely",
  "Libera GPU — scarica i modelli":
    "Free GPU — unload the models",
  "Scarica i modelli dalla VRAM; il server e l'AI restano attivi (si ricaricano al bisogno)":
    "Unloads the models from VRAM; the server and the AI stay on (they reload when needed)",
  "Ferma AI — server e spegni":
    "Stop AI — shut the server down",
  "Scarica i modelli, ferma il server locale e disattiva l'AI":
    "Unloads the models, stops the local server and turns the AI off",
  "Attiva AI":
    "Turn the AI on",
  "Riaccendi l'AI locale (riassunti, tag automatici, domande, wiki…)":
    "Switches the local AI back on (summaries, auto-tags, questions, wiki…)",
  "Backup libreria":
    "Library backup",
  "Copia completa della libreria (PDF + database) in una cartella":
    "A complete copy of the library (PDFs + database) into a folder",
  "I documenti eliminati (ripristinabili)":
    "Deleted documents (they can be restored)",
  "Terminale":
    "Terminal",
  "PowerShell integrato nella cartella dei PDF":
    "A built-in PowerShell, opened in the PDF folder",
  "Plancia":
    "Bridge",
  "La sala macchine: cosa sta lavorando adesso, in tempo reale (finestra separata)":
    "The engine room: what is running right now, in real time (separate window)",
  "La guida di Scriptorium: si sposta e resta aperta (anche in primo piano) mentre lavori":
    "The Scriptorium guide: you can move it and keep it open (even always on top) while you work",
  "11 temi, chiari e scuri":
    "11 themes, light and dark",
  "Sistema":
    "System",
  "Impostazioni":
    "Settings",
  "Controlla aggiornamenti":
    "Check for updates",
  "L'unico momento in cui l'app cerca aggiornamenti: se ce n'è uno ti mostro le novità e decidi tu se installarlo":
    "The only time the app looks for updates: if there is one I show you what's new and you decide whether to install it",
  "Rivedi il benvenuto":
    "Show the welcome again",
  "Ripropone il messaggio di primo avvio (tasto destro, palette, guida)":
    "Brings back the first-launch message (right click, palette, guide)",
  "Informazioni":
    "About",
  "1 DOI recuperato":
    "1 DOI recovered",
  "{n} DOI recuperati":
    "{n} DOIs recovered",
  "1 PDF scaricato":
    "1 PDF downloaded",
  "{n} PDF scaricati":
    "{n} PDFs downloaded",
  "1 aggiornato":
    "1 updated",
  "{n} aggiornati":
    "{n} updated",
  "1 aggiunto":
    "1 added",
  "{n} aggiunti":
    "{n} added",
  "1 aggiunto a «{raccolta}»":
    "1 added to “{raccolta}”",
  "{n} aggiunti a «{raccolta}»":
    "{n} added to “{raccolta}”",
  "1 con PDF":
    "1 with a PDF",
  "{n} con PDF":
    "{n} with PDFs",
  "1 corretto":
    "1 fixed",
  "{n} corretti":
    "{n} fixed",
  "1 da arXiv":
    "1 from arXiv",
  "{n} da arXiv":
    "{n} from arXiv",
  "1 da arXiv (nome file)":
    "1 from arXiv (file name)",
  "{n} da arXiv (nome file)":
    "{n} from arXiv (file name)",
  "1 da confermare a mano (tasto destro → Organizza → Recupera metadati)":
    "1 to confirm by hand (right-click → Organize → Fetch metadata)",
  "{n} da confermare a mano (tasto destro → Organizza → Recupera metadati)":
    "{n} to confirm by hand (right-click → Organize → Fetch metadata)",
  "1 dal testo del PDF":
    "1 from the PDF's own text",
  "{n} dal testo del PDF":
    "{n} from the PDF's own text",
  "1 errore":
    "1 error",
  "{n} errori":
    "{n} errors",
  "1 errore (es. {err})":
    "1 error (e.g. {err})",
  "{n} errori (es. {err})":
    "{n} errors (e.g. {err})",
  "1 file di bibliografia ignorato: trascina PDF, oppure usa Importa → Da bibliografia o progetto…":
    "1 bibliography file ignored: drop PDFs, or use Import → From a bibliography or a project…",
  "{n} file di bibliografia ignorati: trascina PDF, oppure usa Importa → Da bibliografia o progetto…":
    "{n} bibliography files ignored: drop PDFs, or use Import → From a bibliography or a project…",
  "1 già in libreria altrove":
    "1 already in the library elsewhere",
  "{n} già in libreria altrove":
    "{n} already in the library elsewhere",
  "1 già presente":
    "1 already there",
  "{n} già presenti":
    "{n} already there",
  "1 importato":
    "1 imported",
  "{n} importati":
    "{n} imported",
  "1 non trovato":
    "1 not found",
  "{n} non trovati":
    "{n} not found",
  "1 novità su {tot} risultati":
    "1 new paper out of {tot} results",
  "{n} novità su {tot} risultati":
    "{n} new papers out of {tot} results",
  "1 nuova scoperta, in catena da «{da}»":
    "1 new discovery, chained from “{da}”",
  "{n} nuove scoperte, in catena da «{da}»":
    "{n} new discoveries, chained from “{da}”",
  "1 nuovo paper trovato":
    "1 new paper found",
  "{n} nuovi paper trovati":
    "{n} new papers found",
  "1 riferimento collegato":
    "1 reference linked",
  "{n} riferimenti collegati":
    "{n} references linked",
  "1 risolto online per titolo":
    "1 resolved online by title",
  "{n} risolti online per titolo":
    "{n} resolved online by title",
  "1 risultato":
    "1 result",
  "{n} risultati":
    "{n} results",
  "1 saltata (troppo grande o illeggibile)":
    "1 skipped (too big or unreadable)",
  "{n} saltate (troppo grandi o illeggibili)":
    "{n} skipped (too big or unreadable)",
  "1 saltato (già presente)":
    "1 skipped (already done)",
  "{n} saltati (già presenti)":
    "{n} skipped (already done)",
  "1 senza copia Open Access":
    "1 with no Open Access copy",
  "{n} senza copia Open Access":
    "{n} with no Open Access copy",
  "1 senza testo":
    "1 with no text",
  "{n} senza testo":
    "{n} with no text",
  "1 stella fantasma trovata — tratteggiata attorno al paper":
    "1 ghost star found — dashed, around the paper",
  "{n} stelle fantasma trovate — tratteggiate attorno al paper":
    "{n} ghost stars found — dashed, around the paper",
  "1 tag":
    "1 tag",
  "{n} tag":
    "{n} tags",
  "1 tuo paper aggiunto":
    "1 of your papers added",
  "{n} tuoi paper aggiunti":
    "{n} of your papers added",
  "{n} immagini inserite ✓":
    "{n} images inserted ✓",
  "{trovati} PDF allegati su {totale}":
    "{trovati} PDFs attached out of {totale}",
  "AI attivata ✓":
    "AI enabled ✓",
  "AI attivata — il server non risponde: avvialo dalle Impostazioni (clic sul chip AI)":
    "AI enabled — the server is not answering: start it from Settings (click the AI chip)",
  "AI fermata: modelli scaricati, server arrestato, AI disattivata — il chip «AI off» in alto la riattiva":
    "AI stopped: models unloaded, server shut down, AI disabled — the “AI off” chip at the top turns it back on",
  "AI: {err}":
    "AI: {err}",
  "Aggancio dagli appunti…":
    "Grabbing from the clipboard…",
  "Aggiungo alla libreria…":
    "Adding to the library…",
  "Aggiunta non riuscita: {err}":
    "Could not add it: {err}",
  "Aggiunto (solo metadati) ✓":
    "Added (metadata only) ✓",
  "Aggiunto a «{raccolta}»":
    "Added to “{raccolta}”",
  "Aggiunto alla libreria (PDF scaricato) ✓":
    "Added to the library (PDF downloaded) ✓",
  "Aggiunto alla libreria ✓ — entrerà nel grafo al prossimo aggiornamento dell'indice":
    "Added to the library ✓ — it joins the graph at the next index update",
  "Aggiunto col PDF ✓":
    "Added with the PDF ✓",
  "Aggiunto come riferimento ✓":
    "Added as a reference ✓",
  "Aggiunto con PDF ✓":
    "Added with a PDF ✓",
  "Altri {n} file mancanti sono nella stessa cartella spostata. Li ricollego tutti? Verifico l'impronta di ciascuno: chi non corrisponde resta com'è.":
    "Another {n} missing files sit in the same moved folder. Relink them all? Each fingerprint is checked: whatever does not match is left alone.",
  "Un altro file mancante è nella stessa cartella spostata. Lo ricollego? Verifico l'impronta: se non corrisponde resta com'è.":
    "One more missing file sits in the same moved folder. Relink it? The fingerprint is checked: if it does not match, the file is left alone.",
  "Appunti vuoti — copia prima il link del PDF":
    "The clipboard is empty — copy the PDF link first",
  "Appunto esportato in HTML ✓":
    "Note exported to HTML ✓",
  "Appunto esportato in LaTeX ✓ (con le figure)":
    "Note exported to LaTeX ✓ (figures included)",
  "Appunto esportato in Markdown ✓":
    "Note exported to Markdown ✓",
  "Appunto rinominato ✓":
    "Note renamed ✓",
  "Apri la stampa: scegli «Salva come PDF» per esportare":
    "The print dialog is open: choose “Save as PDF” to export",
  "Arresto non riuscito: {err}":
    "Could not stop it: {err}",
  "Avvio del server {nome}…":
    "Starting the {nome} server…",
  "Avvio non riuscito: {err}":
    "Could not start it: {err}",
  "Backup in corso…":
    "Backing up…",
  "Backup non valido: {err}":
    "Invalid backup: {err}",
  "Backup salvato: {percorso}":
    "Backup saved: {percorso}",
  "BibTeX copiato ✓":
    "BibTeX copied ✓",
  "BibTeX copiato (1 voce)":
    "BibTeX copied (1 entry)",
  "BibTeX copiato ({n} voci)":
    "BibTeX copied ({n} entries)",
  "Bibliografia o progetto (BibTeX / RIS / CSL-JSON / .zip)":
    "Bibliography or project (BibTeX / RIS / CSL-JSON / .zip)",
  "Bookmarklet copiato — crea un preferito e incolla questo come indirizzo":
    "Bookmarklet copied — make a bookmark and paste this as its address",
  "Cartella sorvegliata attiva":
    "Watched folder on",
  "Cerco altri lavori dell'autore…":
    "Looking for other work by the author…",
  "Cerco citazioni collegate…":
    "Looking for linked citations…",
  "Cerco novità negli archivi…":
    "Looking for new papers in the archives…",
  "Cerco paper simili…":
    "Looking for similar papers…",
  "Chiave rimossa dal keychain":
    "Key removed from the keychain",
  "Chiave salvata nel keychain ✓":
    "Key saved in the keychain ✓",
  "Chiudere il terminale? La sessione in corso verrà terminata.":
    "Close the terminal? The running session will be killed.",
  "Citazione copiata ✓":
    "Citation copied ✓",
  "Citazioni copiate ({n})":
    "Citations copied ({n})",
  "Citekey copiata: {citekey}":
    "Citekey copied: {citekey}",
  "Confronto: {err}":
    "Comparison failed: {err}",
  "Controllo aggiornamenti…":
    "Checking for updates…",
  "Controllo aggiornamenti: {err}":
    "Update check failed: {err}",
  "Controllo non riuscito: GitHub non raggiungibile (sei offline?)":
    "Check failed: GitHub is unreachable (are you offline?)",
  "Copia non riuscita":
    "Copy failed",
  "Copiato con [@citekey]":
    "Copied with [@citekey]",
  "Copiato con \\cite{…}":
    "Copied with \\cite{…}",
  "DOI copiato ✓":
    "DOI copied ✓",
  "DOI riferimenti: {ris} risolti su {scan} citazioni ({righe} righe aggiornate) — {rest} citazioni ancora senza DOI.":
    "Reference DOIs: {ris} resolved out of {scan} citations ({righe} rows updated) — {rest} citations still have no DOI.",
  "Dal browser: errore nel download":
    "From the browser: download error",
  "Dal browser: già presente in libreria":
    "From the browser: already in the library",
  "Dal browser: il link non è un PDF diretto":
    "From the browser: the link is not a direct PDF",
  "Database":
    "Database",
  "Eliminare definitivamente questo documento? L'operazione è irreversibile.":
    "Delete this document for good? There is no undo.",
  "Eliminare il tag «{nome}»? Verrà tolto da tutti i documenti.":
    "Delete the tag “{nome}”? It is removed from every document.",
  "Eliminare l'appunto «{titolo}»?":
    "Delete the note “{titolo}”?",
  "Eliminare la raccolta «{nome}»? I documenti restano in libreria (le eventuali sotto-raccolte risalgono di un livello).":
    "Delete the collection “{nome}”? The documents stay in the library (any sub-collections move up one level).",
  "Eliminare la ricerca salvata «{nome}»?":
    "Delete the saved search “{nome}”?",
  "Eliminare questa pagina wiki? I documenti non vengono toccati.":
    "Delete this wiki page? The documents are left untouched.",
  "Era già in libreria":
    "It was already in the library",
  "Errore AI: {err}":
    "AI error: {err}",
  "Errore OCR: {err}":
    "OCR failed: {err}",
  "Errore Trova PDF: {err}":
    "Find PDF failed: {err}",
  "Errore backup: {err}":
    "Backup failed: {err}",
  "Errore connettore: {err}":
    "Connector error: {err}",
  "Errore copia: {err}":
    "Copy failed: {err}",
  "Errore esplora citazioni: {err}":
    "Citation explorer failed: {err}",
  "Errore export Obsidian: {err}":
    "Obsidian export failed: {err}",
  "Errore gap citazioni: {err}":
    "Citation gap finder failed: {err}",
  "Errore import LaTeX: {err}":
    "LaTeX import failed: {err}",
  "Errore import bibliografia: {err}":
    "Bibliography import failed: {err}",
  "Errore indice: {err}":
    "Index error: {err}",
  "Errore indicizzazione: {err}":
    "Indexing failed: {err}",
  "Errore metadati: {err}":
    "Metadata error: {err}",
  "Errore nel caricare gli appunti: {err}":
    "Error loading the notes: {err}",
  "Errore nel caricare le novità: {err}":
    "Error loading the new papers: {err}",
  "Errore nel creare l'appunto: {err}":
    "Error creating the note: {err}",
  "Errore nel salvare l'appunto: {err}":
    "Error saving the note: {err}",
  "Errore nel salvataggio della lista: {err}":
    "Error saving the list: {err}",
  "Errore nell'aprire l'appunto: {err}":
    "Error opening the note: {err}",
  "Errore nella rigenerazione delle anteprime: {err}":
    "Error rebuilding the covers: {err}",
  "Errore percorsi: {err}":
    "Error reading the paths: {err}",
  "Errore preferiti: {err}":
    "Favorite error: {err}",
  "Errore ricerca novità: {err}":
    "New-paper sweep failed: {err}",
  "Errore riferimenti: {err}":
    "References error: {err}",
  "Errore rinomina: {err}":
    "Rename failed: {err}",
  "Errore riparazione: {err}":
    "Repair failed: {err}",
  "Errore risoluzione DOI: {err}":
    "DOI resolution failed: {err}",
  "Errore salute libreria: {err}":
    "Library health check failed: {err}",
  "Errore salvataggio chiave: {err}":
    "Error saving the key: {err}",
  "Errore salvataggio ricerca: {err}":
    "Error saving the search: {err}",
  "Errore stampa: {err}":
    "Printing failed: {err}",
  "Errore: {err}":
    "Error: {err}",
  "Esplorazione non riuscita: {err}":
    "Exploration failed: {err}",
  "Esportata 1 nota in Obsidian ({cartella}/Scriptorium)":
    "1 note exported to Obsidian ({cartella}/Scriptorium)",
  "Esportate {n} note in Obsidian ({cartella}/Scriptorium)":
    "{n} notes exported to Obsidian ({cartella}/Scriptorium)",
  "Esportato 1 riferimento ({formato})":
    "1 reference exported ({formato})",
  "Esportati {n} riferimenti ({formato})":
    "{n} references exported ({formato})",
  "Esportazione in PDF non riuscita: {err}":
    "PDF export failed: {err}",
  "Esportazione non riuscita: {err}":
    "Export failed: {err}",
  "GPU liberata (nessun modello era caricato) ✓":
    "GPU freed (no model was loaded) ✓",
  "GPU liberata — 1 modello scaricato dalla VRAM ✓":
    "GPU freed — 1 model unloaded from VRAM ✓",
  "GPU liberata — {n} modelli scaricati dalla VRAM ✓":
    "GPU freed — {n} models unloaded from VRAM ✓",
  "Generazione indice semantico… (al primo uso scarica il modello bge-m3 ~2.3GB)":
    "Building the semantic index… (the first run downloads the bge-m3 model, ~2.3 GB)",
  "Già presente":
    "Already there",
  "HF: {err}":
    "HF: {err}",
  "Il link non era un PDF diretto: salvato come riferimento (controlla l'URL)":
    "The link was not a direct PDF: saved as a reference (check the URL)",
  "Immagine inserita ✓":
    "Image inserted ✓",
  "Immagine non inserita: hai cambiato appunto durante la lettura":
    "Image not inserted: you switched notes while it was being read",
  "Importato dalla cartella sorvegliata: {nome} ✓":
    "Imported from the watched folder: {nome} ✓",
  "Importato un PDF dalla cartella sorvegliata ✓":
    "A PDF was imported from the watched folder ✓",
  "Importazione di 1 file…":
    "Importing 1 file…",
  "Importazione di {n} file…":
    "Importing {n} files…",
  "Importo il progetto LaTeX…":
    "Importing the LaTeX project…",
  "Importo la bibliografia e cerco i PDF…":
    "Importing the bibliography and hunting for the PDFs…",
  "Importo la bibliografia…":
    "Importing the bibliography…",
  "Impossibile attivare l'AI: il salvataggio non è riuscito — riprova dalle Impostazioni":
    "Cannot enable the AI: the save did not go through — try again from Settings",
  "Impossibile attivare l'AI: {err}":
    "Cannot enable the AI: {err}",
  "Impossibile caricare il README: {err}":
    "Cannot load the README: {err}",
  "Impossibile liberare la GPU: {err}":
    "Cannot free the GPU: {err}",
  "Impostazioni salvate":
    "Settings saved",
  "Impostazioni salvate con errori — {dettaglio}":
    "Settings saved, with errors — {dettaglio}",
  "Indice semantico: +1 documento":
    "Semantic index: +1 document",
  "Indice semantico: +{n} documenti":
    "Semantic index: +{n} documents",
  "La libreria è vuota: importa qualche PDF prima":
    "The library is empty: import some PDFs first",
  "La lista è vuota — niente da salvare":
    "The list is empty — nothing to save",
  "LaTeX: {dettaglio}":
    "LaTeX: {dettaglio}",
  "Lista salvata in {percorso}":
    "List saved to {percorso}",
  "Log Plancia: {err}":
    "Bridge log: {err}",
  "Mappa semantica: {err}":
    "Semantic map: {err}",
  "Markdown":
    "Markdown",
  "Markdown copiato":
    "Markdown copied",
  "Metadati applicati ✓":
    "Metadata applied ✓",
  "Metadati: {dettaglio}":
    "Metadata: {dettaglio}",
  "Nessun DOI recuperato su {scan} citazioni — {rest} ancora senza DOI.":
    "No DOI recovered out of {scan} citations — {rest} still have none.",
  "Nessun PDF da stampare (i riferimenti senza file sono stati saltati)":
    "No PDF to print (references without a file were skipped)",
  "Nessun PDF letto di recente.":
    "No PDF read recently.",
  "Nessun metadato errato trovato — tutto a posto ✓":
    "No wrong metadata anywhere — all clean ✓",
  "Nessun nuovo risultato da questa scoperta":
    "Nothing new from this discovery",
  "Nessun nuovo risultato per questa relazione":
    "Nothing new for this relation",
  "Nessun riferimento senza PDF qui":
    "No reference without a PDF here",
  "Nessun tag con almeno 2 documenti: assegna qualche tag prima":
    "No tag has at least 2 documents: assign a few tags first",
  "Nessuna immagine inserita (troppo grande o illeggibile)":
    "No image inserted (too big or unreadable)",
  "Nessuna novità ({tot} risultati)":
    "Nothing new ({tot} results)",
  "Nessuna novità al momento":
    "Nothing new right now",
  "Niente da citare nella selezione":
    "Nothing to cite in the selection",
  "Niente da esportare":
    "Nothing to export",
  "Niente da inserire":
    "Nothing to insert",
  "Non riesco a leggere gli appunti: incolla con Ctrl+V":
    "Cannot read the clipboard: paste with Ctrl+V",
  "Non riesco a preparare il ripristino: {err}":
    "Cannot stage the restore: {err}",
  "Non riesco a salvare la lingua delle risposte AI: {err}":
    "Cannot save the AI answer language: {err}",
  "Non sembra un PDF diretto — usa il link che termina in .pdf":
    "That does not look like a direct PDF — use the link ending in .pdf",
  "Non trovo questo DOI in libreria (rigenera l'esplorazione?)":
    "This DOI is nowhere in the library (regenerate the exploration?)",
  "OCR completato: {car} caratteri da {pag} pagine ✓":
    "OCR done: {car} characters from {pag} pages ✓",
  "OCR in corso… (può richiedere qualche secondo per documenti lunghi)":
    "Running OCR… (a few seconds on long documents)",
  "OCR: {car} caratteri dalle prime {pag} di {tot} pagine (limite) ✓":
    "OCR: {car} characters from the first {pag} of {tot} pages (the cap) ✓",
  "OpenAlex non riconosce questa scoperta":
    "OpenAlex does not recognize this discovery",
  "OpenAlex non riconosce questo paper (né per DOI né per titolo)":
    "OpenAlex does not recognize this paper (neither by DOI nor by title)",
  "PDF agganciato alla libreria ✓":
    "PDF grabbed into the library ✓",
  "PDF agganciato dagli appunti ✓":
    "PDF grabbed from the clipboard ✓",
  "PDF agganciato dal browser ✓":
    "PDF grabbed from the browser ✓",
  "PDF allegato al riferimento ✓":
    "PDF attached to the reference ✓",
  "PDF trovato e allegato ✓":
    "PDF found and attached ✓",
  "Pagina wiki «{concetto}» generata ✓":
    "Wiki page “{concetto}” generated ✓",
  "Pagina wiki «{concetto}» generata da 1 fonte scelta ✓":
    "Wiki page “{concetto}” generated from the 1 source you picked ✓",
  "Pagina wiki «{concetto}» generata dalle {n} fonti scelte ✓":
    "Wiki page “{concetto}” generated from the {n} sources you picked ✓",
  "Percorso copiato ✓":
    "Path copied ✓",
  "Percorso di lettura: {err}":
    "Reading path: {err}",
  "Preparazione stampa…":
    "Preparing to print…",
  "Progetto LaTeX (.zip)":
    "LaTeX project (.zip)",
  "Quel PDF è già in libreria (in un altro documento): usa Strumenti → Duplicati per unirli":
    "That PDF is already in the library, under another document: use Tools → Duplicates to merge them",
  "Quel link non è un PDF diretto":
    "That link is not a direct PDF",
  "Questa scoperta non ha autori registrati":
    "This discovery has no authors on record",
  "Questa scoperta non ha un titolo da cercare":
    "This discovery has no title to search for",
  "Questo documento ha già un PDF":
    "This document already has a PDF",
  "Questo documento non ha un titolo da copiare":
    "This document has no title to copy",
  "Questo elemento non ha un PDF da stampare":
    "This item has no PDF to print",
  "Questo paper non ha autori registrati":
    "This paper has no authors on record",
  "Questo paper non ha un titolo da cercare":
    "This paper has no title to search for",
  "Questo riferimento non ha un file PDF":
    "This reference has no PDF file",
  "Rassegna sui primi 10 di {n} paper selezionati (il massimo)":
    "Reviewing the first 10 of the {n} papers you selected (the maximum)",
  "Rassegna: {err}":
    "Review failed: {err}",
  "Recupero 1 riferimento…":
    "Fetching 1 reference…",
  "Recupero {n} riferimenti…":
    "Fetching {n} references…",
  "Recupero metadati dei documenti incompleti (arXiv dal nome file, DOI e titolo dal PDF)…":
    "Recovering metadata for the incomplete documents (arXiv from the file name, DOI and title from the PDF)…",
  "Riassunti: 1 completato":
    "Summaries: 1 done",
  "Riassunti: {n} completati":
    "Summaries: {n} done",
  "Riassunto generato — aprilo da \"Modifica metadati\"":
    "Summary generated — open it from “Edit metadata”",
  "Riassunto in corso… (può richiedere un momento)":
    "Summarizing… (this can take a moment)",
  "Ricerca salvata: «{nome}»":
    "Search saved: “{nome}”",
  "Ricollega tutti":
    "Relink them all",
  "Ricollegato ✓":
    "Relinked ✓",
  "Ricollegato ✓ — attenzione: il file scelto non è identico all'originale":
    "Relinked ✓ — careful: the file you picked is not identical to the original",
  "Ricollegato anche 1 altro file ✓":
    "1 more file relinked as well ✓",
  "Ricollegati anche {n} file ✓":
    "{n} more files relinked as well ✓",
  "Ricostruire l'indice da zero? I passaggi verranno rigenerati — utile per ottenere le pagine sui documenti già indicizzati.":
    "Rebuild the index from scratch? Every passage is regenerated — worth it to get page numbers on documents indexed earlier.",
  "Ricostruisci":
    "Rebuild",
  "Riferimento aggiunto alla libreria ✓ (usa «Allega PDF…» o Trova PDF per il file)":
    "Reference added to the library ✓ (use “Attach a PDF…” or Find PDF for the file itself)",
  "Rigenerazione anteprime in corso…":
    "Rebuilding the covers…",
  "Rigenero il riassunto… (quello esistente verrà sostituito)":
    "Regenerating the summary… (the existing one is replaced)",
  "Rimosso da «{dove}»":
    "Removed from “{dove}”",
  "Rimuovere questa chiave API dal keychain?":
    "Remove this API key from the keychain?",
  "Rimuovi":
    "Remove",
  "Ripara":
    "Repair",
  "Riparazione completata: {dettaglio}":
    "Repair done: {dettaglio}",
  "Ripristina e riavvia":
    "Restore and restart",
  "Ripristinare da questo backup?\n\n• {docs} documenti\n• {contenuto}\n\nLa libreria ATTUALE verrà sostituita (ne viene salvata prima una copia di sicurezza in backups/). L'app si riavvierà per applicare il ripristino.":
    "Restore from this backup?\n\n• {docs} documents\n• {contenuto}\n\nYour CURRENT library will be replaced (a safety copy of it is saved first, under backups/). The app restarts to apply the restore.",
  "Ripristino pronto: verrà applicato al prossimo avvio di Scriptorium.":
    "Restore staged: it is applied the next time Scriptorium starts.",
  "Risultati: {err}":
    "Results: {err}",
  "Salvataggio dell'appunto aperto non riuscito — riprova prima di mandarci del testo":
    "Could not save the open note — try again before sending text into it",
  "Salvataggio non riuscito: riprova prima di cambiare appunto":
    "Save failed: try again before switching notes",
  "Salvataggio non riuscito: riprova prima di creare un appunto":
    "Save failed: try again before creating a note",
  "Salvataggio non riuscito: riprova prima di esportare":
    "Save failed: try again before exporting",
  "Salvataggio non riuscito: riprova prima di rinominare":
    "Save failed: try again before renaming",
  "Salvato negli Appunti ✓":
    "Saved to Notes ✓",
  "Sei aggiornato: {attuale} è l'ultima versione":
    "You are up to date: {attuale} is the latest version",
  "Seleziona 2 o 3 documenti da confrontare":
    "Select 2 or 3 documents to compare",
  "Seleziona da 2 a 10 paper per una rassegna":
    "Select 2 to 10 papers for a review",
  "Seleziona i documenti da cui raccogliere i risultati":
    "Select the documents to harvest results from",
  "Serve un DOI o almeno un titolo per esplorare le citazioni":
    "A DOI, or at least a title, is needed to explore the citations",
  "Serve un DOI o almeno un titolo per le citazioni":
    "A DOI, or at least a title, is needed for the citations",
  "Serve un link diretto al PDF che inizia con https://":
    "A direct link to the PDF is needed, one starting with https://",
  "Sposta nel cestino":
    "Move to trash",
  "Spostare 1 documento nel cestino?":
    "Move 1 document to the trash?",
  "Spostare {n} documenti nel cestino?":
    "Move {n} documents to the trash?",
  "Stampa avviata":
    "Printing started",
  "Stampa avviata: 1 documento":
    "Printing started: 1 document",
  "Stampa avviata: {n} documenti":
    "Printing started: {n} documents",
  "Stampa avviata: 1 documento · {salt} senza PDF saltati":
    "Printing started: 1 document · {salt} without a PDF skipped",
  "Stampa avviata: {n} documenti · {salt} senza PDF saltati":
    "Printing started: {n} documents · {salt} without a PDF skipped",
  "Svuota cestino":
    "Empty the trash",
  "Svuotare il cestino? 1 documento verrà eliminato definitivamente.":
    "Empty the trash? 1 document is deleted for good.",
  "Svuotare il cestino? {n} documenti verranno eliminati definitivamente.":
    "Empty the trash? {n} documents are deleted for good.",
  "Tag aggiunti: {elenco}":
    "Tags added: {elenco}",
  "Tag automatici in corso…":
    "Auto-tagging…",
  "Tag automatici: 1 completato":
    "Auto-tag: 1 done",
  "Tag automatici: {n} completati":
    "Auto-tag: {n} done",
  "Titolo copiato ✓":
    "Title copied ✓",
  "Trova PDF: {dettaglio}":
    "Find PDF: {dettaglio}",
  "Tutti i selezionati hanno già dei tag":
    "Everything selected already has tags",
  "Tutti i selezionati hanno già un riassunto AI (per rifarne uno: tasto destro sul singolo documento)":
    "Everything selected already has an AI summary (to redo one: right-click that single document)",
  "Unire {n} documenti in uno? Gli altri verranno rimossi (note e tag confluiscono nel primo).":
    "Merge {n} documents into one? The others are removed (their notes and tags flow into the first).",
  "Unisci":
    "Merge",
  "Uniti {n} documenti":
    "{n} documents merged",
  "Verifica e riparazione in corso… (può richiedere fino a un minuto)":
    "Checking and repairing… (this can take up to a minute)",
  "Verifico ogni documento e correggo quelli il cui titolo non corrisponde al PDF (di solito causati dal DOI di un lavoro citato). Per ognuno cerco il record giusto online per titolo (Crossref/arXiv); i paper arXiv si recuperano dall'id nel nome file; se non è indicizzato da nessuna parte, ricavo almeno il titolo dalla prima riga del PDF. I documenti già corretti non vengono toccati. Può richiedere fino a un minuto. Procedo?":
    "Every document is checked, and the ones whose title does not match their PDF are fixed (usually the DOI of a cited work leaked in). For each one the right record is looked up online by title (Crossref/arXiv); arXiv papers are recovered from the id in the file name; if it is indexed nowhere, at least the title is taken from the PDF's first line. Documents that are already correct are left alone. This can take up to a minute. Go ahead?",
  "Wiki aggiornata: 1 pagina ✓":
    "Wiki updated: 1 page ✓",
  "Wiki aggiornata: {n} pagine ✓":
    "Wiki updated: {n} pages ✓",
  "Wiki: {err}":
    "Wiki: {err}",
  "\\cite copiato (1 chiave)":
    "\\cite copied (1 key)",
  "\\cite copiato ({n} chiavi)":
    "\\cite copied ({n} keys)",
  "interrotto":
    "stopped",
  "libreria completa (database + PDF + note + progetti)":
    "the full library (database + PDFs + notes + projects)",
  "nessun PDF nel .zip":
    "no PDF in the .zip",
  "nessuna voce trovata nel file":
    "no entry found in the file",
  "niente da importare":
    "nothing to import",
  "progetto LaTeX creato":
    "LaTeX project created",
  "raccolta «{nome}»":
    "collection “{nome}”",
  "ricerca online: {err}":
    "online search: {err}",
  "solo il database (catalogo, tag, grafo, annotazioni)":
    "the database only (catalog, tags, graph, annotations)",
  "{formato}: {dettaglio}":
    "{formato}: {dettaglio}",
  "{nome}: avviato":
    "{nome}: started",
  "{nome}: avvio richiesto":
    "{nome}: start requested",
  "{nome}: fermato":
    "{nome}: stopped",
  "È disponibile Scriptorium {nuova} (hai la {attuale}) — scaricala da GitHub col segnalino in alto":
    "Scriptorium {nuova} is available (you have {attuale}) — download it from GitHub via the marker at the top",
  "✓ 1 anteprima rigenerata ad alta risoluzione":
    "✓ 1 cover rebuilt at high resolution",
  "✓ {n} anteprime rigenerate ad alta risoluzione":
    "✓ {n} covers rebuilt at high resolution",

  // ======================================================================
  // Da qui in giù: messaggi del backend (src-tauri).
  //
  // Il Rust resta scritto in ITALIANO e non traduce mai: traduce il consumatore,
  // con `te()`. Queste chiavi quindi non compaiono come `t("…")` nel sorgente
  // TypeScript — `scripts/i18n-check.mjs` le verifica contro i sorgenti Rust.
  // Non spostare questo marcatore: è ciò che divide le due metà del file.
  // ======================================================================
  "Abilita prima «Scopri online» nelle Impostazioni.":
    "Turn on “Discover online” in Settings first.",
  "Cartella del vault non valida":
    "Invalid vault folder",
  "Dai un nome al progetto":
    "Give the project a name",
  "Domanda mancante":
    "No question given",
  "Esiste già un progetto con questo nome":
    "A project with this name already exists",
  "File troppo grande":
    "File too large",
  "File troppo grande per l'editor":
    "File too large for the editor",
  "Formato immagine non supportato":
    "Unsupported image format",
  "Funzione disponibile solo su Windows":
    "This feature is only available on Windows",
  "Generazione annullata":
    "Generation cancelled",
  "Gerarchia troppo profonda: spostamento rifiutato":
    "Hierarchy too deep: move refused",
  "Il candidato non ha né identificativo né titolo":
    "The candidate has neither an identifier nor a title",
  "Il documento ha già del testo estratto; OCR annullato":
    "This document already has extracted text; OCR cancelled",
  "Il documento ha già del testo estratto; OCR annullato per non sovrascriverlo":
    "This document already has extracted text; OCR cancelled so it is not overwritten",
  "Il file di backup è danneggiato (controllo integrità fallito): non verrà usato.":
    "The backup file is damaged (integrity check failed): it will not be used.",
  "Il file scelto non esiste (o è una cartella)":
    "The file you picked does not exist (or it is a folder)",
  "Il modello non ha prodotto un riassunto":
    "The model produced no summary",
  "Il modello non ha prodotto una risposta":
    "The model produced no answer",
  "Il modello non ha prodotto una tabella valida":
    "The model produced no valid table",
  "Il modello non ha restituito LaTeX":
    "The model returned no LaTeX",
  "Il modello non ha restituito testo":
    "The model returned no text",
  "Il modello non ha restituito una tabella":
    "The model returned no table",
  "Il modello non ha riconosciuto una struttura di tabella in quest'area: restringi la selezione alla sola tabella, o prova il motore «Nativa».":
    "The model found no table structure in this area: narrow the selection down to the table alone, or try the “Native” engine.",
  "Il nome del tag non può essere vuoto":
    "The tag name cannot be empty",
  "Il nome della raccolta è vuoto":
    "The collection name is empty",
  "Il titolo non può essere vuoto":
    "The title cannot be empty",
  "Immagine non riconosciuta (atteso un data-URL base64)":
    "Image not recognized (a base64 data URL was expected)",
  "Indirizzo locale non consentito":
    "Local addresses are not allowed",
  "La cartella dello specchio non ha il marker di Scriptorium: riattivalo dall'Archivio":
    "The mirror folder has no Scriptorium marker: switch the mirror back on from the Archive",
  "La cartella non è vuota e non è uno specchio di Scriptorium: scegline una vuota o dedicata":
    "The folder is not empty and is not a Scriptorium mirror: pick an empty or dedicated one",
  "La cartella scelta non contiene «pdfmanage.db»: non è un backup di Scriptorium.":
    "The folder you picked does not contain “pdfmanage.db”: it is not a Scriptorium backup.",
  "La raccolta di destinazione non esiste":
    "The destination collection does not exist",
  "La raccolta madre non esiste":
    "The parent collection does not exist",
  "La ricerca online è disattivata (abilitala in Impostazioni → Ricerca online)":
    "Online search is off (turn it on in Settings → Online search)",
  "La ricerca online è disattivata (abilitala nelle impostazioni)":
    "Online search is off (turn it on in the settings)",
  "La ricerca online è disattivata: attivala in Impostazioni → Ricerca online":
    "Online search is off: turn it on in Settings → Online search",
  "La tabella è troppo grande per il modello di visione (output troncato). Usa il motore «Nativa» per tabelle grandi.":
    "The table is too large for the vision model (output truncated). Use the “Native” engine for large tables.",
  "Le funzioni AI sono disattivate (abilitale nelle Impostazioni)":
    "The AI features are off (turn them on in Settings)",
  "Le raccolte smart si popolano da sole (non si trascina dentro)":
    "Smart collections fill themselves (you cannot drag into them)",
  "Le raccolte smart si popolano da sole con la loro regola":
    "Smart collections fill themselves from their own rule",
  "Le raccolte smart si popolano già da sole con la loro regola":
    "Smart collections already fill themselves from their own rule",
  "Lo specchio è spento (attivalo nell'Archivio)":
    "The mirror is off (switch it on in the Archive)",
  "Nessun contenuto pertinente trovato nei documenti. L'indice dei passaggi è costruito? (Chiedi alla libreria → Costruisci indice)":
    "No relevant content found in the documents. Is the passage index built? (Ask your library → Build index)",
  "Nessun documento per questo concetto: usa il nome di un tag esistente o genera l'indice semantico":
    "No document for this concept: use the name of an existing tag, or generate the semantic index",
  "Nessun documento valido nella selezione":
    "No valid document in the selection",
  "Nessun file .bib trovato nell'archivio (uno .zip di Overleaf deve contenerne almeno uno)":
    "No .bib file found in the archive (an Overleaf .zip must contain at least one)",
  "Nessun file PDF su disco per questo documento":
    "No PDF file on disk for this document",
  "Nessun modello di visione selezionato":
    "No vision model selected",
  "Nessun passaggio trovato. Costruisci l'indice (Chiedi alla libreria → Costruisci indice) e verifica che i PDF contengano testo.":
    "No passage found. Build the index (Ask your library → Build index) and check that the PDFs actually contain text.",
  "Nessun prerequisito trovato: servono i riferimenti (chip «✦ senza metadati» in alto) o l'indice semantico":
    "No prerequisite found: this needs the references (the “✦ missing metadata” chip at the top) or the semantic index",
  "Nessun risultato quantitativo trovato nei documenti selezionati":
    "No quantitative result found in the selected documents",
  "Nessun testo disponibile da riassumere per questo documento":
    "No text available to summarize for this document",
  "Nessun testo disponibile per generare i tag":
    "No text available to generate the tags",
  "Nessun testo nel PDF in quest'area (pagina scansionata?): il modello struttura ha bisogno del testo del PDF per riempire le celle — usa il motore «Ollama».":
    "No text in the PDF in this area (a scanned page?): the structure model needs the PDF's own text to fill the cells — use the “Ollama” engine.",
  "Nessun testo selezionato":
    "No text selected",
  "Nessuna cartella scelta":
    "No folder chosen",
  "Nessuna cartella scelta per lo specchio":
    "No folder chosen for the mirror",
  "Nome nota non valido":
    "Invalid note name",
  "Nome repository non valido":
    "Invalid repository name",
  "Non puoi spostare una raccolta dentro una sua sotto-raccolta":
    "You cannot move a collection into one of its own sub-collections",
  "Non usare la radice di un disco: scegli una sottocartella dedicata":
    "Do not use the root of a drive: pick a dedicated subfolder",
  "OCR non ha riconosciuto testo in questo PDF":
    "OCR recognized no text in this PDF",
  "Ollama non ha restituito l'embedding":
    "Ollama returned no embedding",
  "Operazione non riuscita":
    "The operation failed",
  "Pagina non trovata":
    "Page not found",
  "Prima costruisci l'Indice semantico (icona a strati sulla barra): i suggerimenti usano quello":
    "Build the Semantic index first (the layers icon in the toolbar): the suggestions run on it",
  "Questa è una scheda bibliografica senza PDF: usa «Trova PDF» per allegarne uno":
    "This is a reference with no PDF: use “Find PDF” to attach one",
  "Questa è una scheda bibliografica, non un PDF spostato: usa «Trova PDF» dal menu del documento, così il testo viene estratto e indicizzato":
    "This is a reference, not a PDF that has moved: use “Find PDF” from the document menu, so the text gets extracted and indexed",
  "Questo DOI è già usato da un altro documento":
    "This DOI is already used by another document",
  "Questo elemento non ha un file PDF":
    "This item has no PDF file",
  "Questo file non sembra una libreria di Scriptorium (manca la tabella «documents»).":
    "This file does not look like a Scriptorium library (the “documents” table is missing).",
  "Raccolta non trovata":
    "Collection not found",
  "Record arXiv non trovato per questo identificativo":
    "No arXiv record found for this identifier",
  "Scegli prima la cartella dello specchio":
    "Choose the mirror folder first",
  "Scegli una cartella FUORI dai dati dell'app (%APPDATA%\\com.pdfmanage.app)":
    "Choose a folder OUTSIDE the app's own data (%APPDATA%\\com.pdfmanage.app)",
  "Scegli una cartella di backup (con pdfmanage.db) o un file .db.":
    "Choose a backup folder (one that contains pdfmanage.db) or a .db file.",
  "Scegli una cartella separata dalla cartella sorvegliata (il watcher re-importerebbe lo specchio in loop)":
    "Choose a folder separate from the watched folder (the watcher would re-import the mirror in a loop)",
  "Schema URL non consentito":
    "URL scheme not allowed",
  "Scrivi un concetto (o usa il nome di un tag)":
    "Type a concept (or use the name of a tag)",
  "Scrivi una domanda":
    "Type a question",
  "Seleziona da 1 a 8 documenti":
    "Select from 1 to 8 documents",
  "Senza DOI serve un titolo di almeno 8 caratteri per l'aggancio sicuro su OpenAlex":
    "Without a DOI, a safe match on OpenAlex needs a title of at least 8 characters",
  "Serve un DOI, un id OpenAlex o almeno un titolo per esplorare le citazioni":
    "Exploring the citations needs a DOI, an OpenAlex id or at least a title",
  "Serve un file PDF":
    "A PDF file is required",
  "Servono da 2 a 10 documenti per una rassegna":
    "A review needs between 2 and 10 documents",
  "URL non valido":
    "Invalid URL",
  "URL vuoto":
    "Empty URL",
  "Una raccolta non può stare dentro sé stessa":
    "A collection cannot sit inside itself",
  "Una raccolta smart non può avere sotto-raccolte":
    "A smart collection cannot have sub-collections",
  "cartella dati non disponibile":
    "data folder not available",
  "cartella dell'eseguibile non trovata":
    "the executable's folder was not found",
  "chiave sconosciuta":
    "unknown key",
  "documento non trovato":
    "document not found",
  "estrazione PDF interrotta":
    "PDF extraction was interrupted",
  "impossibile salvare il PDF":
    "cannot save the PDF",
  "nome progetto non valido":
    "invalid project name",
  "novità non trovata":
    "feed item not found",
  "percorso di salvataggio non valido":
    "invalid save path",
  "progetto non trovato":
    "project not found",
  "ricerca non trovata":
    "saved search not found",
  "vault non disponibile":
    "vault not available",
  "ADS richiede un token API (impostalo nelle impostazioni)":
    "ADS requires an API token (set it in the settings)",
  "Analisi PDF":
    "PDF analysis",
  "Apertura .zip":
    "Opening the .zip",
  "Backup non leggibile":
    "Backup not readable",
  "CORE richiede una API key gratuita (impostala nelle impostazioni)":
    "CORE requires a free API key (set it in the settings)",
  "CORE: API key non valida":
    "CORE: invalid API key",
  "CORE: troppe richieste, riprova tra poco":
    "CORE: too many requests, try again shortly",
  "Copia PDF nella libreria":
    "Copying the PDF into the library",
  "Creazione cartella assets":
    "Creating the assets folder",
  "Creazione cartella note":
    "Creating the notes folder",
  "Creazione cartella temp":
    "Creating the temp folder",
  "Creazione nota":
    "Creating the note",
  "Creazione papers/":
    "Creating papers/",
  "Eliminazione nota":
    "Deleting the note",
  "Formato di esportazione non supportato":
    "Unsupported export format",
  "GitHub: limite di richieste raggiunto (imposta un token nelle impostazioni)":
    "GitHub: rate limit reached (set a token in the settings)",
  "Immagini non trovate o troppo grandi per l'export":
    "Images missing or too large to export",
  "LM Studio non raggiungibile (server locale avviato?)":
    "LM Studio unreachable (is the local server running?)",
  "Lettura .zip":
    "Reading the .zip",
  "Lettura file":
    "Reading the file",
  "Lettura immagine":
    "Reading the image",
  "Lettura nota":
    "Reading the note",
  "Non riesco a creare la cartella":
    "Cannot create the folder",
  "Non riesco a leggere il file":
    "Cannot read the file",
  "Non riesco a preparare il ripristino":
    "Cannot prepare the restore",
  "Non riesco ad aprire il backup":
    "Cannot open the backup",
  "OCR è disponibile solo su Windows":
    "OCR is only available on Windows",
  "Ollama non raggiungibile (è in esecuzione?)":
    "Ollama unreachable (is it running?)",
  "Ollama: campo 'embeddings' mancante (modello scaricato? `ollama pull bge-m3`)":
    "Ollama: the 'embeddings' field is missing (is the model downloaded? `ollama pull bge-m3`)",
  "Ollama: embedding non valido":
    "Ollama: invalid embedding",
  "Operazione non supportata":
    "Unsupported operation",
  "PowerShell non disponibile":
    "PowerShell not available",
  "Questo repository non ha un README":
    "This repository has no README",
  "Rinomina file":
    "Renaming the file",
  "Salvataggio immagine":
    "Saving the image",
  "Salvataggio nota":
    "Saving the note",
  "Semantic Scholar: troppe richieste, riprova tra poco (o imposta una API key)":
    "Semantic Scholar: too many requests, try again shortly (or set an API key)",
  "Task fallito":
    "Task failed",
  "apertura .zip":
    "opening the .zip",
  "base64 non valido":
    "invalid base64",
  "client di download":
    "download client",
  "costruisco tensore":
    "building the tensor",
  "costruisco tensore context":
    "building the context tensor",
  "costruisco tensore immagine":
    "building the image tensor",
  "costruisco tensore token":
    "building the token tensor",
  "decoder senza input context":
    "the decoder has no context input",
  "decoder senza input token":
    "the decoder has no token input",
  "decoder senza sequenze":
    "the decoder returned no sequence",
  "decodifico immagine":
    "decoding the image",
  "encoder senza input":
    "the encoder has no input",
  "immagine non valida":
    "invalid image",
  "lettura .zip":
    "reading the .zip",
  "lo .zip non contiene file utilizzabili":
    "the .zip contains no usable file",
  "modello tabella senza input":
    "the table model has no input",
  "motore OCR di Windows non disponibile (installa un language pack OCR dalle impostazioni di Windows)":
    "the Windows OCR engine is not available (install an OCR language pack from the Windows settings)",
  "nessun file .tex con \\documentclass alla radice dello .zip: rinomina il documento principale in main.tex e riprova":
    "no .tex file with \\documentclass at the root of the .zip: rename the main document to main.tex and try again",
  "nessuna formula riconosciuta":
    "no formula recognized",
  "output boxes mancante":
    "the boxes output is missing",
  "output logits mancante":
    "the logits output is missing",
  "pdfium non trovato nelle risorse dell'app o accanto all'eseguibile":
    "pdfium not found in the app resources or next to the executable",
  "percorso non valido":
    "invalid path",
  "richiesta a LM Studio fallita":
    "the request to LM Studio failed",
  "richiesta a Ollama fallita":
    "the request to Ollama failed",
  "richiesta embeddings a Ollama fallita":
    "the embeddings request to Ollama failed",
  "richiesta vision a LM Studio fallita":
    "the vision request to LM Studio failed",
  "richiesta vision a Ollama fallita":
    "the vision request to Ollama failed",
  "rinomina in main.tex":
    "renaming to main.tex",
  "risposta LM Studio non valida":
    "invalid LM Studio response",
  "risposta Ollama non valida":
    "invalid Ollama response",
  "risposta embeddings Ollama non valida":
    "invalid Ollama embeddings response",
  "scarico modello tabelle":
    "downloading the table model",
  "stream interrotto":
    "the stream was interrupted",
  "Aggiunta dalla scoperta":
    "Adding from discovery",
  "Aggiunta per identificatori":
    "Adding by identifiers",
  "Anteprime":
    "Covers",
  "Arricchimento metadati":
    "Metadata enrichment",
  "Backup della libreria":
    "Library backup",
  "Compilazione LaTeX":
    "LaTeX compilation",
  "Confronto AI":
    "AI comparison",
  "Domanda alla libreria":
    "Question to your library",
  "Estrazione PDF fallita":
    "PDF extraction failed",
  "Estrazione tabella":
    "Table extraction",
  "Estrazione tabella (TATR)":
    "Table extraction (TATR)",
  "Formula → LaTeX (OCR locale)":
    "Formula → LaTeX (local OCR)",
  "Generazione wiki":
    "Wiki generation",
  "Import":
    "Import",
  "Import BibTeX":
    "BibTeX import",
  "Import da URL":
    "Import from a URL",
  "Import da gestore bibliografico":
    "Import from a reference manager",
  "Import dal browser":
    "Import from the browser",
  "Import fallito":
    "Import failed",
  "Import progetto LaTeX":
    "LaTeX project import",
  "Import progetto LaTeX (.zip)":
    "LaTeX project import (.zip)",
  "Indice per «Chiedi»":
    "Index for “Ask”",
  "Novità accettata ma non archiviata":
    "Feed item accepted but not filed",
  "Novità accettata: archiviata nella sua raccolta":
    "Feed item accepted: filed in its collection",
  "OCR del documento":
    "Document OCR",
  "Rassegna AI":
    "AI review",
  "Recupero metadati":
    "Metadata retrieval",
  "Riassunto AI del documento":
    "AI summary of the document",
  "Ricerca PDF open-access":
    "Open-access PDF search",
  "Ricollego i file spostati":
    "Relinking the files that moved",
  "Rigenerazione anteprime":
    "Rebuilding the covers",
  "Ripristino: preparazione":
    "Restore: preparation",
  "Ripristino: preparazione dal backup":
    "Restore: preparing from the backup",
  "Risoluzione DOI dei riferimenti":
    "Resolving the DOIs of the references",
  "Scarico modelli formula":
    "Downloading the formula models",
  "Specchio su disco":
    "Disk mirror",
  "Specchio su disco: rigenerazione":
    "Disk mirror: rebuild",
  "Specchio su un altro volume":
    "Mirror on another volume",
  "Suggerimenti":
    "Suggestions",
  "Sweep «Novità»":
    "“What's new” sweep",
  "Verifico i file ritrovati":
    "Checking the files found again",
  "PDF prodotto":
    "PDF produced",
  "compilazione fallita (vedi log del progetto)":
    "compilation failed (see the project log)",
  "interrotto dall'utente":
    "stopped by the user",
  "niente hardlink possibili: verranno fatte copie vere (spazio doppio)":
    "no hardlinks possible here: real copies will be made (twice the disk space)",
  "\n[interrotto: superato il tempo massimo di compilazione]":
    "\n[stopped: the compilation time limit was exceeded]",
  " (nel cestino)":
    " (in the trash)",
  "Citato direttamente dal paper: è un suo fondamento dichiarato":
    "Cited directly by the paper: one of its declared foundations",
  "DOI arXiv della scheda":
    "arXiv DOI on the card",
  "DOI della scheda":
    "DOI on the card",
  "DOI presente nel PDF":
    "DOI present in the PDF",
  "DOI stampato nel PDF":
    "DOI printed in the PDF",
  "La raccolta non ha ancora paper con un vettore semantico: mettine dentro 1-2 (o costruisci l'Indice semantico), oppure usa la modalità NOME":
    "This collection has no paper with a semantic vector yet: put 1-2 of them in it (or build the Semantic index), or use NAME mode",
  "Limite dimensione archivio raggiunto: import interrotto":
    "Archive size limit reached: import stopped",
  "Log della Plancia su file: attivato":
    "Bridge log to file: on",
  "Molto vicino per contenuti e precedente: prepara il contesto":
    "Very close in content and earlier: it sets the context",
  "OpenAlex (dal DOI)":
    "OpenAlex (from the DOI)",
  "PDF non cercati: la ricerca online è disattivata (Impostazioni → Ricerca online)":
    "PDFs not searched: online search is off (Settings → Online search)",
  "Riferimento del paper non ancora in libreria":
    "A reference of the paper, not in your library yet",
  "Semantic Scholar (dal DOI)":
    "Semantic Scholar (from the DOI)",
  "Unpaywall (dal DOI)":
    "Unpaywall (from the DOI)",
  "anno nella prima pagina":
    "year on the first page",
  "arXiv nel nome del file":
    "arXiv id in the file name",
  "arXiv stampato nel PDF":
    "arXiv id printed in the PDF",
  "già in libreria (possibile duplicato)":
    "already in your library (possible duplicate)",
  "id arXiv della scheda":
    "arXiv id on the card",
  "identificativo della scheda":
    "identifier on the card",
  "nome del file":
    "file name",
  "parole del titolo nella prima pagina":
    "title words on the first page",
  "ricerca per titolo (Crossref)":
    "search by title (Crossref)",
  "ricerca per titolo (OpenAlex)":
    "search by title (OpenAlex)",
  "ricerca per titolo (Semantic Scholar)":
    "search by title (Semantic Scholar)",
  "ricerca per titolo (arXiv)":
    "search by title (arXiv)",
  "stesso anno":
    "same year",
  "titolo dal PDF":
    "title from the PDF",
  "titolo della scheda":
    "title on the card",
  "titolo identico a quello nel PDF":
    "title identical to the one in the PDF",
  "titolo identico alla scheda":
    "title identical to the one on the card",
  "via il DOI della scheda":
    "via the DOI on the card",
  ".bib troppo grande, saltato":
    ".bib too large, skipped",
  "PDF troppo grande, saltato":
    "PDF too large, skipped",
  "Progetto LaTeX non creato":
    "LaTeX project not created",
  "batch note":
    "notes batch",
};
