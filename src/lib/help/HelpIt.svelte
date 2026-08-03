<script lang="ts">
  /* Prosa della guida, versione ITALIANA. Nessuna logica e nessun foglio di
     stile: il guscio HelpProse.svelte possiede il wrapper .helpprose e tutti
     gli stili della prosa. Il markup e' copiato verbatim da +page.svelte, cosi'
     l'estrazione resta verificabile riga per riga.

     i18n-exempt: prosa per lingua, non chiavi di dizionario — la traduzione e'
     il file gemello HelpEn.svelte, e il controllo e' la parita' strutturale fra
     i due (stesso numero di h3, li, dt/dd, tr in ciascuna scheda). */
  import type { HelpTab } from "./tabs";

  let { tab }: { tab: HelpTab } = $props();
</script>

        {#if tab === "inizia"}
        <div class="helpsec">
          <h3>Le tre porte d'ingresso</h3>
          <ul>
            <li><strong>Barra strumenti</strong> (in alto): un'icona per ogni strumento — passaci sopra col mouse per il nome. Nell'ordine: <strong>I miei paper</strong> (torna alla griglia), <strong>Importa</strong>, <strong>Vista</strong>, <strong>Riprendi lettura</strong>, <strong>Chiedi alla libreria</strong>, <strong>Wiki</strong>, <strong>Cerca online</strong>, <strong>Appunti</strong>, <strong>Progetti (LaTeX)</strong>, <strong>Archivio</strong> (raccolte e sotto-raccolte, in vista sinottica), <strong>Riscopri</strong>, <strong>Novità</strong> (🔔 col conteggio dei nuovi paper), <strong>Esporta</strong>, <strong>Cura della libreria</strong>, <strong>Indice semantico</strong>, <strong>Memoria AI</strong> (o <em>Attiva AI</em> quando è spenta), <strong>Backup</strong>, <strong>Cestino</strong>, <strong>Terminale</strong> (&gt;_), <strong>Plancia</strong> (il sinottico dei processi, in finestra separata), <strong>Guida</strong>, <strong>Aspetto</strong>, <strong>Sistema</strong> (Impostazioni · Controlla aggiornamenti · Informazioni). Le voci con un menu si aprono al clic, le altre eseguono; l'icona è evidenziata quando sei nella vista corrispondente. In alto trovi anche il chip <strong>AI</strong> (stato dell'AI locale), «✦ N senza metadati» quando serve, e l'icona della <strong>palette</strong>.</li>
            <li><strong>Menu radiale</strong> (tasto destro): su un <strong>documento</strong> → le azioni su quel documento; sullo <strong>spazio vuoto</strong> → il menu globale (gli stessi gruppi della barra); su una <strong>selezione multipla</strong> → le azioni in blocco. Muovi verso un petalo e clicca (basta la direzione); <strong>rotella</strong> per ruotare; <strong>digita</strong> per filtrare tutte le voci a qualsiasi profondità; il centro torna indietro, <kbd>Esc</kbd> chiude. La <strong>descrizione</strong> della voce evidenziata compare sotto l'anello.</li>
            <li><strong>Palette comandi</strong> (<kbd>Ctrl</kbd>+<kbd>K</kbd>): ogni azione, documento, <strong>appunto</strong>, <strong>pagina wiki</strong>, <strong>progetto LaTeX</strong>, filtro, sezione della guida e tema — digitando. Funziona <strong>anche dentro il lettore</strong>. Barra, radiale e palette pescano dallo <strong>stesso registro</strong>: se non trovi un comando, è comunque lì.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>La griglia e la barra laterale</h3>
          <ul>
            <li><strong>Un clic</strong> su una scheda apre il <strong>pannello di dettaglio</strong> a destra (abstract, riassunto AI, tag modificabili, citazioni, note); <strong>doppio clic</strong> o <kbd>Invio</kbd> aprono il lettore.</li>
            <li>La <strong>barra laterale</strong> è la navigazione: filtri rapidi (Preferiti, Da leggere, Con codice, Peer-reviewed, Il mio lavoro), tag, raccolte, ricerche salvate, cartella sorvegliata. <kbd>Ctrl</kbd>+<kbd>B</kbd> la mostra/nasconde.</li>
            <li>Nella vista «Tutti»: la <strong>Panoramica</strong> (da leggere, in lettura, aggiunti questo mese + un paper da riscoprire al giorno) e <strong>Continua a leggere</strong> (gli ultimi PDF aperti). Nel lettore la barra svanisce mentre leggi e lo zoom è ricordato per documento.</li>
            <li><strong>Questa guida è una finestra</strong>: trascinala dalla barra del titolo, ridimensionala dall'angolo in basso a destra (come quasi tutte le finestre di dialogo) e spunta <strong>«in primo piano»</strong> per tenerla visibile — anche sopra il lettore — mentre segui i passaggi.</li>
          </ul>
          <table class="kbdtable">
            <tbody>
              <tr><td><kbd>Ctrl</kbd>+<kbd>K</kbd></td><td>Palette comandi — tutto, digitando</td></tr>
              <tr><td><kbd>/</kbd></td><td>Ricerca in libreria</td></tr>
              <tr><td><kbd>Ctrl</kbd>+<kbd>B</kbd></td><td>Mostra/nascondi la barra laterale</td></tr>
              <tr><td><kbd>frecce</kbd> / <kbd>Spazio</kbd></td><td>Muoviti tra le schede / apri-chiudi il pannello</td></tr>
              <tr><td><kbd>X</kbd> / <kbd>F</kbd></td><td>Seleziona / preferito</td></tr>
              <tr><td><kbd>Invio</kbd> / <kbd>Esc</kbd></td><td>Apri il lettore / chiudi</td></tr>
              <tr><td>Tasto destro</td><td>Menu radiale (documento, selezione o globale)</td></tr>
              <tr><td><kbd>Tab</kbd> → <kbd>Invio</kbd></td><td>Raggiungi la barra strumenti e apri il suo menu</td></tr>
              <tr><td><kbd>↑</kbd> <kbd>↓</kbd> nei menu</td><td>Scorri le voci — <kbd>Esc</kbd> chiude e torna all'icona</td></tr>
            </tbody>
          </table>
          <p class="dimtext">Tutto è raggiungibile da tastiera: nell'<strong>Archivio</strong> lo schema delle raccolte prende il fuoco con <kbd>Tab</kbd>, poi <kbd>↑</kbd><kbd>↓</kbd> scorrono l'elenco, <kbd>→</kbd><kbd>←</kbd> entrano ed escono dalle sotto-raccolte e <kbd>Invio</kbd> apre la raccolta nella griglia.</p>
        </div>

        {:else if tab === "libreria"}
        <div class="helpsec">
          <h3>Importare</h3>
          <ul>
            <li><strong>Sei vie</strong> (barra → Importa): <strong>PDF dal disco</strong> (anche trascinandoli nella finestra — restano dove sono, l'app li indicizza; i duplicati si riconoscono dal contenuto); <strong>Da bibliografia o progetto</strong> — l'export di <strong>Zotero, Mendeley, EndNote, JabRef…</strong> in <strong>.bib / .ris / CSL-JSON</strong>: porta metadati, aggancia i PDF (dal campo <em>file</em> o da una cartella d'export che indichi) e trasforma le parole chiave in <strong>tag</strong>, senza doppioni (dedup per DOI e per contenuto del PDF; i lavori già nel Cestino tornano visibili); per <strong>identificatore</strong> (DOI / arXiv / ISBN / PMID); <strong>da URL</strong>; <strong>progetto LaTeX (.zip)</strong> — i tuoi paper con la loro bibliografia, marcati «Il mio lavoro»; <strong>Cartella sorvegliata</strong> (importa da sola ciò che ci finisce dentro).</li>
            <li><strong>Dal browser</strong>: copia il link del PDF e torna su Scriptorium — compare «Aggancia» (interruttore in Impostazioni → Connettore); in alternativa il <strong>bookmarklet</strong>, o la Cartella sorvegliata puntata su Download.</li>
            <li><strong>Riferimenti senza PDF</strong> (aggiunti da ricerca online, citazioni, BibTeX o ID): <strong>Trova PDF…</strong> (radiale della scheda, o aprendo la voce) mostra i <strong>candidati</strong> trovati online per identificativo e per titolo (arXiv, Unpaywall, OpenAlex, Semantic Scholar, Crossref) con le prove — «Scarica e allega» quello giusto, «Apri pagina» per controllare, o incolla un link diretto. Sulla <strong>selezione multipla</strong> e in blocco (Cura della libreria → «Trova PDF dei riferimenti») resta automatico: allega solo abbinamenti sicuri, ora anche per titolo su arXiv/S2.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Organizzare</h3>
          <ul>
            <li><strong>Tag</strong> colorati (la <strong>✎</strong> in sidebar rinomina/ricolora, la <strong>×</strong> elimina; dal pannello dettagli li applichi al volo) e <strong>Raccolte</strong>, anche <em>smart</em> (si popolano da sole con una regola).</li>
            <li><strong>Filtri</strong> in sidebar (Preferiti, Da leggere, Con codice, Peer-reviewed, Il mio lavoro), <strong>ordinamento combinabile</strong> (chip «Ordina ▾»: un clic attiva, un altro inverte, un terzo toglie), badge <em>preprint / peer-reviewed</em> sulle schede.</li>
            <li><strong>Viste</strong> (barra → Vista): griglia (copertine ridimensionabili con − ▭ +), lista a colonne, <strong>Costellazione</strong> (la mappa semantica — vedi la scheda <em>Scoperta</em>). Clic su un <strong>autore</strong> → tutti i suoi lavori.</li>
            <li><strong>Archivio</strong> (icona cartella sulla barra): le raccolte come <strong>albero navigabile</strong> — sotto-raccolte a piacere, <strong>trascina un paper</strong> su una raccolta per spostarlo (Ctrl = aggiungi anche lì: l'appartenenza è multipla; <strong>sullo sfondo vuoto</strong> = toglilo dalla raccolta), trascina una raccolta su un'altra per annidarla. Eliminare una raccolta non tocca mai i paper (le sotto-raccolte risalgono). Puoi aprire la <strong>Costellazione della sola raccolta</strong> in tre modi: il pulsante <strong>✳</strong> accanto al nome della raccolta in sidebar, la voce <em>Costellazione di «nome»</em> nel menu <strong>Vista</strong> mentre la stai guardando, o il pulsante <strong>COSTELLAZIONE</strong> nel pannello dell'Archivio. Le vicinanze vengono ricalcolate al suo interno e il layout è salvato a parte dalla mappa generale. Nel pannello: <strong>✦ Suggerisci</strong> propone i paper affini (somiglianza semantica locale, con soglia di confidenza — mai automatico): scegli la sorgente <em>prima</em> di calcolare — <em>Nome</em> della raccolta (funziona anche a motori spenti: si àncora ai tuoi paper che ne contengono le parole), <em>Contenuto</em> (i paper già dentro) o <em>Entrambi</em> col <strong>peso regolabile</strong>. Il toggle <strong>Ricerca «Novità»</strong> aggancia una ricerca online alla raccolta (le novità accettate <em>entrano da sole nella raccolta</em>, filtrate per pertinenza quando la raccolta ha ≥3 paper indicizzati; spegnendolo la ricerca si rimuove).</li>
            <li>Nel pannello di ogni raccolta c'è anche la sezione <strong>Appunti</strong>: gli appunti <code>.md</code> agganciati a quella raccolta, più <strong>«+ Appunto con i N paper»</strong> che ne crea uno già agganciato e con i paper elencati come citazioni. Serve a non ritrovarsi un elenco piatto di sintesi che si perdono; i dettagli sono nella scheda <em>Scrittura</em>.</li>
            <li><strong>Specchio su disco</strong> (chip in alto nell'Archivio): proietta le raccolte in una cartella vera — <code>Raccolta\Sottoraccolta\Autore Anno — Titolo.pdf</code> — con <strong>hardlink</strong> (zero spazio extra), aggiornata da sola a ogni cambio. Comodissima da Esplora risorse e dal terminale. Cancellare o spostare file nello specchio non tocca la libreria (si rigenera); <em>modificare il contenuto</em> di un PDF lì dentro sì, perché è lo stesso file: per annotare usa il lettore.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Condividere e stampare</h3>
          <ul>
            <li>Tasto destro → <strong>Condividi</strong>: <strong>WhatsApp / Teams / Gmail</strong> aprono la bozza col messaggio pronto e il <strong>PDF è già copiato ✓ di sistema</strong> — incollalo con <kbd>Ctrl</kbd>+<kbd>V</kbd>; <strong>Outlook desktop</strong> allega il file da solo. Funziona anche sulla <strong>selezione multipla</strong>.</li>
            <li><strong>Stampa</strong>: dal lettore (menu <strong>⋯ Altro</strong> o radiale) per il documento aperto, o dal radiale della selezione per stamparne più d'uno.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Metadati</h3>
          <ul>
            <li>Quando ci sono schede incomplete compare «<strong>✦ N senza metadati</strong>»: un clic le recupera in blocco (con barra di avanzamento e Stop) — prima l'id arXiv nel <strong>nome del file</strong> (funziona anche sulle scansioni), poi DOI e <strong>titolo</strong> dal PDF (Crossref/arXiv). Mai dal primo DOI trovato nel testo: niente etichette sbagliate — ciò che non è sicuro resta com'è.</li>
            <li>Per il caso singolo o ostinato: tasto destro → Organizza → <strong>Recupera metadati…</strong> fa la ricerca <em>estesa</em> (Crossref, arXiv, OpenAlex, ogni DOI/arXiv stampato nel PDF, nome del file) e mostra i <strong>candidati con le prove</strong> trovate nel PDF (titolo, autori, anno): scegli tu quale applicare, o incolla un DOI/arXiv. Lo stesso da Salute libreria («Trova…») accanto a ogni documento incompleto.</li>
            <li><strong>Impostazioni → Manutenzione → «Verifica e ripara metadati»</strong> ricontrolla tutta la libreria e corregge le schede il cui titolo non corrisponde al PDF. Sicuro e ripetibile. A mano: tasto destro → <strong>Modifica metadati</strong>.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Cura, backup, cestino</h3>
          <ul>
            <li><strong>Cura della libreria</strong> (barra): <strong>Salute</strong> (file mancanti, PDF senza testo → OCR, metadati incompleti), <strong>Gap di citazioni</strong> (i DOI più citati dai tuoi paper che non possiedi; «Risolvi DOI dei riferimenti» ne recupera altri online, senza mai un abbinamento incerto), <strong>Duplicati</strong> (unione), <strong>Rigenera anteprime</strong>.</li>
            <li><strong>Backup libreria</strong> (barra): copia completa — PDF + database — in una cartella a tua scelta. <strong>Cestino</strong>: gli eliminati restano ripristinabili finché non lo svuoti.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Cercare in libreria</h3>
          <ul>
            <li>Barra di ricerca in alto (<kbd>/</kbd>), tre modalità: <em>Tutto</em> (testo + semantica), <em>Testo</em> o <em>Semantica</em> (per significato — serve l'<strong>Indice semantico</strong>). Cerca anche nelle <strong>annotazioni</strong> e nelle <strong>note dei documenti</strong>; gli <strong>Appunti .md</strong> che corrispondono compaiono in un gruppo dedicato sopra i risultati.</li>
          </ul>
        </div>

        {:else if tab === "scoperta"}
        <div class="helpsec">
          <h3>Cerca online</h3>
          <ul>
            <li>Barra → <strong>Cerca online</strong>: arXiv, OpenAlex, ADS, Semantic Scholar, Europe PMC, CORE, DOAJ, <strong>HF Papers</strong> (l'indice con i repo GitHub dei paper). Filtri anno/autore/solo-OA, chip <strong>Con codice</strong> / <strong>Peer-reviewed</strong> / <strong>Preprint</strong>, colonne ordinabili. I PDF Open Access si <strong>scaricano</strong>, gli altri entrano come <strong>riferimento</strong> (allegherai il PDF dopo).</li>
            <li><strong>Ricerche salvate → Novità</strong>: dopo una ricerca premi <em>★ Salva</em> → va in sidebar. I risultati <em>nuovi</em> dall'ultima volta si raccolgono nella campana <strong>Novità</strong> (🔔, ricontrollata a ogni avvio): da lì <em>Aggiungi</em> o <em>Ignora</em>.</li>
            <li><strong>Riscopri</strong> (barra): ti ripesca un documento dimenticato o mai letto.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Codice, citazioni e repository</h3>
          <ul>
            <li>I paper che citano un repo mostrano l'icona <strong>GitHub</strong> (card, lista, risultati online): cliccala per aprire il repository.</li>
            <li>Tasto destro → <strong>Codice & repo</strong>: anteprima del <strong>README</strong> nell'app, più i modelli/dataset collegati su <strong>Hugging Face</strong>.</li>
            <li>Filtro <strong>“Con codice (GitHub)”</strong> nella sidebar per vedere solo i paper con codice disponibile.</li>
            <li>Tasto destro → Cita → <strong>Riferimenti e citazioni</strong>: la bibliografia del paper (con i riferimenti già nella tua libreria cliccabili) e i documenti che lo <strong>citano</strong>. Lì trovi anche <strong>Copia APA / IEEE / BibTeX / citekey / \cite / [@…]</strong>. Con più paper <strong>selezionati</strong>, tasto destro → Cita copia un solo <code>\cite&#123;k1,k2&#125;</code> o tutte le voci BibTeX insieme (per LaTeX/Pandoc).</li>
            <li><strong>Esplora citazioni (online)</strong> si apre sulla <strong>Mappa</strong>: riferimenti a sinistra (il passato su cui si fonda), citazioni a destra (il futuro), in ordine di anno; pallino <strong>pieno</strong> = già in libreria, <strong>tratteggiato</strong> = mancante, nodo più grande = paper più citato. Clic su un nodo → aggiungi / apri / <em>esplora da qui</em> (la mappa si ricentra, «← Indietro» per tornare). La <strong>Lista</strong> con tutte le azioni (+ PDF, salva) resta a un click.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Costellazione — la mappa della libreria</h3>
          <ul>
            <li>Barra → Vista → <strong>Costellazione</strong>: ogni <strong>stella</strong> è un documento, i <strong>rombi viola</strong> sono i tuoi appunti, i legami sono la <strong>somiglianza di significato</strong>. Serve l'<strong>Indice semantico</strong>. Se qualche paper non ha ancora l'incorporamento, in alto compare <strong>«✦ N fuori mappa»</strong>: premilo e l'indice si genera <em>da lì</em>, con la mappa che si ridisegna da sola a fine lavoro — non serve uscire e rientrare. Il pulsante <strong>↻</strong> ricarica il grafo (utile dopo aver cambiato la densità), e la mappa si aggiorna comunque da sola quando importi o elimini documenti. Le posizioni si ricordano tra le sessioni.</li>
            <li><strong>Clic</strong> = scheda del nodo (con i legami in %, cliccabili); <strong>doppio clic</strong> = apri; <strong>tasto destro</strong> = menu; <strong>Ctrl+clic</strong> = seleziona. Da lontano vedi le <strong>nebulose</strong> — le comunità semantiche con la loro etichetta; da vicino compaiono i badge <strong>✓</strong> (peer-reviewed) e la forcella (codice GitHub).</li>
            <li>In alto a destra: <strong>Cerca nel grafo</strong> (bastano 2-3 lettere del titolo o di un autore → candidati suggeriti; Invio o clic evidenzia la stella con un alone pulsante e centra la vista lì; × o Esc pulisce), <strong>Colora per</strong> (Tag dominante / Comunità semantiche / Anno / Stato lettura), <strong>Nebulose</strong> e il pannello <strong>⚙</strong> (Legami per nodo, Soglia somiglianza).</li>
            <li><strong>Esplora dintorni (online)</strong>, nella scheda di una stella: <em>Citazioni</em> / <em>Simili</em> / <em>Autore</em> → i risultati appaiono come <strong>stelle fantasma</strong> tratteggiate attorno a quella; clic → «Aggiungi alla libreria». Entrano nel grafo al prossimo aggiornamento dell'indice.</li>
            <li><strong>Catena di esplorazione</strong>: anche la scheda di una <em>stella fantasma</em> ha Citazioni / Simili / Autore — le nuove scoperte si agganciano a quella, in catena (snowball), così scavi di paper in paper senza aggiungere nulla finché non trovi quello giusto. Mentre esplori la mappa entra in <strong>modalità esplorazione</strong>: la libreria si attenua, i seed hanno un anello «scanner», ogni generazione della catena ha il suo colore e i collegamenti scorrono animati; le catene si dispongono da sole senza sovrapporsi. <strong>Citazioni</strong> funziona anche senza DOI (id OpenAlex per le scoperte; titolo con corrispondenza rigorosa per i tuoi paper).</li>
          </ul>
        </div>

        {:else if tab === "lettura"}
        <div class="helpsec">
          <h3>Il lettore</h3>
          <ul>
            <li><strong>Immersivo</strong>: la barra svanisce mentre leggi (torna col mouse), gli strumenti rari sono sotto <strong>⋯ Altro</strong>, lo zoom è ricordato per documento e si riparte dall'<strong>ultima pagina</strong>. «<strong>Riprendi lettura</strong>» (barra/radiale/palette) riapre l'ultimo PDF al punto in cui eri.</li>
            <li><strong>Annotazioni</strong> (<kbd>A</kbd>): evidenziazioni con colore e commento, o <strong>note puntuali</strong> «a spillo» — ancorate alla pagina. <strong>Nota del documento</strong> (<kbd>E</kbd>): un appunto per l'<em>intero</em> paper. Gli <strong>Appunti .md</strong> sono invece file indipendenti (scheda <em>Scrittura</em>).</li>
            <li><strong>Ritrovare un'evidenziazione senza ricordare in quale paper era</strong>: la ricerca in alto guarda anche <em>dentro</em> le tue evidenziazioni e i tuoi commenti — i risultati compaiono nel gruppo «<strong>Evidenziazioni</strong>» e un clic apre il PDF alla pagina esatta. Il segno <strong>✎ n</strong> sulla scheda dice quante ne contiene, senza aprirla.</li>
            <li><strong>Hai spostato o rinominato un PDF fuori da Scriptorium?</strong> Aprendolo trovi «<strong>Ritrova il file…</strong>»: indichi dov'è ora e la scheda resta intatta (evidenziazioni, tag, nota, citekey). Se hai spostato un'<em>intera cartella</em>, dopo il primo ti propone di sistemare in un colpo tutti gli altri, verificando l'impronta di ciascun file. Lo stesso pulsante è in <em>Cura della libreria → File mancanti sul disco</em>.</li>
            <li><strong>Cerca nel documento</strong> (<kbd>Ctrl</kbd>+<kbd>F</kbd>) in una fascia dedicata; <kbd>Invio</kbd>/<kbd>Maiusc</kbd>+<kbd>Invio</kbd> scorrono i risultati. Più indice, zoom/adatta, rotazione, due pagine, modalità notte.</li>
            <li><strong>Lente AI</strong>: seleziona un passaggio → <em>Spiega</em> / <em>Traduci</em> / <em>Chiedi</em> (AI locale attiva); la risposta si può salvare nella Nota del documento.</li>
            <li><strong>Manda agli Appunti</strong>: selezione → radiale o barretta dell'evidenziazione → scegli l'appunto → entra come citazione col riferimento <code>[[@citekey]]</code> in coda. Le <strong>fonti numerate</strong> di «Chiedi alla libreria» aprono il PDF alla pagina giusta.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Estrarre dal PDF: tabelle, testo, formule, figure</h3>
          <ul>
            <li>Premi il tasto della modalità (<kbd>T</kbd> tabella, <kbd>X</kbd> testo, <kbd>F</kbd> formula, <kbd>G</kbd> figura — o le icone/⋯) e <strong>trascina un rettangolo</strong> sulla zona. La finestra che si apre è trascinabile e ridimensionabile; ovunque hai <strong>Copia / Salva / → Appunti</strong> con il <strong>formato a scelta</strong> (l'anteprima mostra esattamente cosa esce). Ciò che mandi agli Appunti porta con sé il riferimento <code>[[@citekey]]</code>.</li>
            <li><strong>Tabella</strong> (<kbd>T</kbd>) — tre motori nell'intestazione: <strong>Nativa</strong> (dal testo del PDF, veloce, per tabelle semplici), <strong>Modello</strong> (riconosce righe/colonne/intestazioni dall'immagine e prende il testo <em>esatto</em> dal PDF — il migliore sui paper; ~111 MB scaricati al primo uso; non lavora su pagine ruotate), <strong>Ollama</strong> (modello di visione, per scansioni). Export: griglia, CSV, Markdown, Excel, LaTeX (booktabs), più «Migliora con AI».</li>
            <li><strong>Testo</strong> (<kbd>X</kbd>) — <strong>Nativa</strong> conserva la <strong>formattazione</strong> (corsivo, grassetto, apici/pedici → Markdown, es. H<sub>2</sub>O); <strong>Ollama</strong> fa l'OCR delle pagine scansionate. Export: testo, Markdown, LaTeX.</li>
            <li><strong>Formula</strong> (<kbd>F</kbd>) — motore <strong>Locale</strong> (math-OCR integrato, ~115 MB al primo uso) o <strong>Ollama</strong>; «<strong>Più righe</strong>» per equazioni impilate. Il LaTeX riconosciuto è <strong>modificabile</strong> con anteprima resa in tempo reale; il pulsante <code>\mathrm&#123;&#125;</code> mette tutto in tondo. Export: LaTeX o <code>$$…$$</code>.</li>
            <li><strong>Figura</strong> (<kbd>G</kbd>) — ritaglia qualsiasi zona come immagine: <strong>Salva PNG…</strong> o <strong>→ Appunti</strong>.</li>
          </ul>
          <table class="kbdtable">
            <tbody>
              <tr><td><kbd>Ctrl</kbd>+<kbd>F</kbd></td><td>Cerca nel documento</td></tr>
              <tr><td><kbd>+</kbd> / <kbd>−</kbd> / <kbd>0</kbd></td><td>Ingrandisci / riduci / zoom 100%</td></tr>
              <tr><td><kbd>W</kbd> / <kbd>H</kbd></td><td>Adatta alla larghezza / alla pagina</td></tr>
              <tr><td><kbd>2</kbd></td><td>Vista a due pagine</td></tr>
              <tr><td><kbd>N</kbd></td><td>Aggiungi una nota puntuale</td></tr>
              <tr><td><kbd>A</kbd> / <kbd>E</kbd></td><td>Pannello Annotazioni / Nota del documento</td></tr>
              <tr><td><kbd>T</kbd> / <kbd>X</kbd> / <kbd>F</kbd> / <kbd>G</kbd></td><td>Estrai tabella / testo / formula / figura (ripremi per annullare)</td></tr>
              <tr><td><kbd>I</kbd></td><td>Modalità notte (inverti colori)</td></tr>
              <tr><td><kbd>[</kbd> / <kbd>]</kbd></td><td>Ruota a sinistra / destra</td></tr>
              <tr><td><kbd>Ctrl</kbd>+rotella</td><td>Zoom continuo</td></tr>
              <tr><td><kbd>Esc</kbd></td><td>Chiudi / annulla</td></tr>
              <tr><td><kbd>?</kbd></td><td>Scorciatoie (dentro il lettore)</td></tr>
            </tbody>
          </table>
        </div>

        {:else if tab === "scrittura"}
        <div class="helpsec">
          <h3>Wiki della libreria</h3>
          <ul>
            <li>La tua <strong>enciclopedia privata</strong>: una pagina per concetto, scritta dall'AI locale leggendo solo i tuoi documenti (icona <strong>Wiki</strong> sulla barra).</li>
            <li>Le fonti, in tre modi: <strong>«Genera/aggiorna dai tag»</strong> (una pagina per ogni tag con almeno 2 documenti); un <strong>concetto libero</strong> (i paper pertinenti li trova la ricerca semantica); oppure <strong>scegli tu le fonti</strong> — seleziona i documenti nella griglia → tasto destro → <em>Pagina wiki (AI)</em>: la pagina usa esattamente quelli (max 10).</li>
            <li>Le citazioni <strong>[n]</strong> nel testo aprono il PDF <strong>alla pagina giusta</strong>; i concetti citati in altre pagine diventano <strong>link</strong> tra pagine; in fondo trovi le fonti con i passaggi usati (chip «p. N»).</li>
            <li>Ogni fonte <em>deve</em> comparire nella pagina: se la sintesi non la usa, viene dichiarata in «Fonti non integrate» — mai omessa in silenzio. Il pallino <strong>●</strong> sull'elenco segnala che la libreria è cambiata e conviene rigenerare.</li>
            <li>Richiede l'AI locale attiva e l'indice dei passaggi (Chiedi alla libreria → Costruisci indice). Consiglio: un modello ≥ 8B (es. <code>gemma3:27b</code>) per una prosa all'altezza.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Appunti</h3>
          <ul>
            <li><strong>File .md veri</strong> nella cartella dell'app: li apri e modifichi anche da un editor esterno o dal terminale (l'indice si riallinea a ogni avvio). Icona <strong>Appunti</strong> sulla barra; elenco ordinabile per modifica/creazione/titolo. <em>Diversi</em> dalle Annotazioni (ancorate al PDF) e dalla Nota del documento (una per paper).</li>
            <li><strong>[[Collegamenti]]</strong>: <code>[[Titolo di un appunto]]</code> collega un appunto; <code>[[@citekey]]</code> o <code>[[Titolo di un paper]]</code> collegano un documento (clic → si apre). In fondo i <strong>backlink</strong>. Cercabili dalla barra di ricerca.</li>
            <li><strong>Editor</strong> con salvataggio automatico e tre modalità: <em>Modifica</em>, <em>Affiancato</em> (anteprima live), <em>Anteprima</em>. Barra di formattazione (titoli, grassetto, liste, sposta blocchi), <strong>formule</strong> con <code>$$ … $$</code> (rese in anteprima), <strong>immagini</strong> trascinate o incollate (salvate come file in <code>assets/</code>, nel testo resta un riferimento breve). <strong>Rinomina</strong> (o doppio clic sul titolo) rinomina anche il file.</li>
            <li><strong>Agganciare un appunto a una raccolta</strong> — così un appunto che discute un gruppo di paper si trova <em>dentro</em> quel gruppo, invece che in fondo a un elenco che cresce a ogni sintesi. Due strade: dal pulsante <strong>«Raccolta…»</strong> sopra l'editor (per un appunto che hai già scritto), oppure dall'<strong>Archivio</strong>, dove il pannello di ogni raccolta ha la sezione <strong>Appunti</strong>. Un appunto può stare in più raccolte. In elenco la raccolta compare come etichetta, e il menu <strong>Raccolta</strong> filtra (c'è anche «Senza raccolta», per ritrovare quelli mai sistemati).</li>
            <li>Dall'Archivio, <strong>«+ Appunto con i N paper»</strong> crea l'appunto <em>già agganciato</em> e con i paper della raccolta elencati come citazioni <code>[[@citekey]]</code>: le citekey non sono a schermo — stanno nella scheda di ogni paper — quindi l'elenco è la parte noiosa che ti risparmi. Solo l'elenco: nessuna struttura imposta, perché quella è tua. Sganciare un appunto, o eliminare la raccolta, <strong>non tocca il file .md</strong>.</li>
            <li><strong>Esporta</strong> (sotto il titolo): <strong>MD</strong> · <strong>HTML</strong> (pagina autonoma) · <strong>LaTeX</strong> (.tex + figure in cartella) · <strong>PDF</strong> — formule e figure incluse.</li>
            <li>Gli appunti <strong>ricevono</strong> dal lettore: testo selezionato, tabelle, formule e figure estratte, sempre con il riferimento <code>[[@citekey]]</code>; dal pannello dettagli anche Abstract e Riassunto AI; le <strong>Rassegne AI</strong> si salvano come appunto. Con l'Indice semantico, gli appunti compaiono anche nella <strong>Costellazione</strong> (rombi viola).</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Progetti (LaTeX) — un piccolo Overleaf locale</h3>
          <ul>
            <li>Icona <strong>Progetti (LaTeX)</strong> sulla barra. Ogni progetto è una <strong>cartella vera</strong> (in <code>projects/</code> nei dati dell'app) con <code>main.tex</code> e <code>refs.bib</code>. Crea da <strong>5 modelli</strong> (articolo, paper a due colonne, relazione/tesi, presentazione beamer, minimale) oppure «<strong>Da .zip…</strong>» con un template scaricato — i link a <strong>Overleaf / IEEE / ACM / Springer / Elsevier</strong> sono lì sotto.</li>
            <li><strong>Cita</strong>: cerca nella tua libreria e inserisce <code>\cite&#123;citekey&#125;</code> al cursore. <strong>Sincronizza bibliografia</strong>: riscrive <code>refs.bib</code> con tutta la libreria. Salvataggio automatico (o <kbd>Ctrl</kbd>+<kbd>S</kbd>).</li>
            <li><strong>Compila</strong> usa il compilatore di sistema: <strong>Tectonic</strong>, oppure <strong>MiKTeX</strong> (via texify, non serve Perl), oppure latexmk. L'anteprima del PDF appare accanto all'editor; se il PDF esce con avvisi lo vedi comunque, col log a un clic. Senza compilatore: <code>winget install Tectonic.Tectonic</code>.</li>
                      <li><strong>Portare qui un progetto Overleaf</strong>: «Da .zip (anche Overleaf)…» ricostruisce l'intero progetto — .tex, immagini, classi, sottocartelle — pronto da compilare; lo stesso lo fa la terza domanda dell'import «Da bibliografia o progetto…».</li>
            <li><strong>Sincronizzare con Overleaf</strong>: con il ponte Git di Overleaf (funzione a pagamento) puoi fare <code>git clone</code> della cartella del progetto dal <strong>Terminale</strong> integrato e lavorare con <code>git pull</code>/<code>git push</code> — le credenziali restano nel gestore di Windows, mai dentro Scriptorium. Il pulsante <strong>+ .gitignore</strong> prepara la cartella escludendo i prodotti della compilazione. Senza il ponte, si passa dallo <code>.zip</code> (vedi la FAQ su Overleaf).</li>
</ul>
        </div>

        <div class="helpsec">
          <h3>Strumenti di sintesi sulla selezione</h3>
          <ul>
            <li><strong>Confronta (AI)</strong>: seleziona 2-3 paper → tasto destro → <em>Confronta</em>: tabella obiettivo/metodo/dati/risultati/limiti + cosa aggiunge ciascuno.</li>
            <li><strong>Rassegna (AI)</strong>: <strong>seleziona da 2 a 10 paper</strong> → tasto destro → <em>Rassegna (AI)</em>. Ottieni una <strong>mini related-work</strong> (300-500 parole) <strong>organizzata per temi</strong> (non paper-per-paper): confronta gli approcci, evidenzia disaccordi e chiude con <em>«Lacune aperte»</em>. Ogni paper è citato con <strong>[n] cliccabili</strong> (aprono la fonte) e nessuna fonte viene omessa in silenzio (le non integrate finiscono in coda). Puoi copiarla pronta per <strong>LaTeX</strong> (<code>\cite&#123;citekey&#125;</code>) o <strong>Pandoc</strong> (<code>[@citekey]</code>), oppure <strong>«Salva negli Appunti»</strong>: diventa un <strong>appunto .md</strong> dove le <em>[n]</em> sono riscritte come <strong>backlink <code>[[@citekey]]</code></strong> — così ogni paper citato rimanda alla rassegna, ed è cercabile e modificabile come ogni appunto. Richiede l'AI locale attiva.</li>
            <li><strong>Tabella risultati (AI)</strong>: raccoglie i numeri (metodo · dataset · metrica · valore) dei paper selezionati in un'unica tabella esportabile in CSV/Markdown/Excel. I valori sono estratti testualmente: verifica sempre sul PDF.</li>
            <li><strong>Percorso di lettura</strong> (tasto destro → AI): per capire un paper, cosa leggere prima — i fondamenti che cita (già tuoi), i vicini di contenuto precedenti, e i riferimenti mancanti da aggiungere con un click. Funziona <em>senza</em> LLM.</li>
          </ul>
        </div>

        {:else if tab === "ai"}
        <div class="helpsec">
          <h3>AI locale (opzionale, mai automatica)</h3>
          <ul>
            <li>Serve <strong>Ollama</strong> o <strong>LM Studio</strong> sul tuo PC: configura URL e modello in <strong>Impostazioni → AI locale</strong>. Il <strong>chip AI</strong> in alto dice lo stato: se leggi «AI off», un clic la riattiva. Consiglio: un modello ≥ 8B (es. <code>gemma3:27b</code>).</li>
            <li>Cosa abilita: <strong>riassunti</strong> e <strong>tag automatici</strong>, <strong>Lente AI</strong> nel lettore, <strong>Chiedi alla libreria</strong>, <strong>Wiki</strong>, <strong>Confronta/Rassegna</strong>, i motori <strong>Ollama</strong> di tabelle/testo/formule, «Migliora con AI» sulle tabelle. Le schede con riassunto mostrano <strong>✦ AI</strong>; il batch sulla selezione <strong>salta</strong> chi ha già riassunto/tag.</li>
            <li><strong>Chiedi alla libreria</strong> (barra): fai una domanda in linguaggio naturale → risposta con <strong>fonti numerate</strong> che aprono i PDF alla pagina giusta. Alla prima visita premi «<strong>Costruisci indice</strong>» (l'indice dei passaggi, riusato anche dalla Wiki); rigeneralo quando la libreria cambia molto.</li>
            <li><strong>Memoria AI</strong> (barra): «<strong>Libera GPU</strong>» scarica i modelli dalla VRAM lasciando tutto attivo (si ricaricano al bisogno); «<strong>Ferma AI</strong>» scarica i modelli, chiude davvero il server locale e disattiva l'AI.</li>
            <li>L'<strong>Indice semantico</strong> (barra) non è l'AI generativa: è l'indice dei significati, tutto locale. Abilita ricerca <em>Semantica</em>, <em>Correlati</em>, <strong>Costellazione</strong> e gli appunti nel grafo. Rilancialo dopo aver aggiunto documenti o appunti.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>I tuoi dati — tutto locale</h3>
          <ul>
            <li>Cartella dell'app: <code>%APPDATA%\com.pdfmanage.app</code> — <code>pdfmanage.db</code> (il catalogo), <code>papers/</code> (i PDF scaricati; quelli importati dal disco <em>restano dove sono</em>), <code>notes/</code> (gli appunti .md + <code>assets/</code>), <code>projects/</code> (i progetti LaTeX), <code>thumbnails/</code> e i modelli locali (<code>mathocr</code>, <code>tablestruct</code>, <code>fastembed_cache</code>).</li>
            <li>Appunti e progetti sono <strong>file veri</strong>: modificarli da fuori è previsto. <strong>Backup libreria</strong> (barra) fa una copia completa; <strong>Esporta</strong> produce citazioni (BibTeX/RIS/CSL) o note per <strong>Obsidian</strong>.</li>
            <li><strong>Aggiornamenti — solo quando li chiedi tu.</strong> L'app non contatta GitHub da sola: l'unica verifica parte da <em>Sistema → Controlla aggiornamenti</em>. Se ce n'è uno ti mostro <strong>cosa cambia</strong> e decidi: «Scarica e installa» mostra l'avanzamento, installa (una finestrella, nessuna domanda) e ti chiede se riavviare. Ogni aggiornamento è <strong>firmato</strong>: uno non firmato con la chiave di Scriptorium viene rifiutato. La tua libreria non viene toccata — sta in <code>%APPDATA%</code>, separata dal programma — e prima di ogni cambio di versione il database viene comunque salvato in <code>backups/</code>. Dopo il riavvio compare una volta il riquadro con le <strong>novità</strong>.</li>
            <li><strong>Terminale</strong> integrato (&gt;_, es. per <code>claude code</code>); la CLI <code>scriptorium-cli</code> interroga da fuori, in sola lettura, libreria <em>e</em> Appunti <em>e</em> progetti LaTeX (<code>query</code>, <code>bib</code>, <code>notes</code>, <code>note</code>, <code>search-notes</code>, <code>projects</code>, <code>stats</code>…). Il <strong>server MCP</strong> <code>scriptorium-mcp</code> porta gli stessi dati (9 strumenti, sola lettura) dentro <strong>Claude Desktop / Claude Code</strong> e qualsiasi client MCP: config pronta da copiare in <strong>Impostazioni → CLI e MCP</strong>. Il <strong>connettore browser</strong> per «Aggancia» è un servizio solo-locale, spegnibile in Impostazioni. <strong>11 temi</strong> in Aspetto.</li>
          </ul>

          <h3>Plancia — il sinottico dei processi</h3>
          <ul>
            <li>La <strong>Plancia</strong> (icona tachimetro sulla barra, o Ctrl+K → «Plancia») apre una <strong>finestra separata</strong> con un <strong>sinottico visivo</strong> dei processi interni: import, estrazione, metadati, indici, AI, backup… Puoi <strong>tenerla in background</strong> su un lato dello schermo: si illumina <em>solo</em> ciò che sta lavorando davvero, con avanzamento (<code>12/96</code>) e durata; da ferma è spenta.</li>
            <li><strong>Stato ed errori a colpo d'occhio</strong>: un guasto accende il nodo in rosso con il <strong>motivo</strong> per esteso; un problema non bloccante (es. un file su cento) lo segna in ambra senza fermare il resto. I sottosistemi spenti dicono <em>perché</em> («online disattivato», «modelli da scaricare»…). Clic su un nodo: descrizione, statistiche, storico.</li>
            <li>Il <strong>registro attività</strong> in basso è filtrabile (Tutti/Errori) ed esportabile con <strong>Salva registro…</strong>; da <strong>Impostazioni → Manutenzione</strong> puoi farlo scrivere anche <strong>su file</strong> (uno al giorno, conservati 14) per capire a posteriori cosa è successo.</li>
          </ul>
        </div>

        {:else if tab === "faq"}
        <div class="helpsec">
          <h3>Come faccio a…?</h3>
          <dl class="faq">
            <dt>…aggiungere il PDF che ho appena scaricato col browser?</dt>
            <dd>Copia il link del PDF e torna su Scriptorium: compare «Aggancia». Oppure punta la Cartella sorvegliata su Download: entra da solo.</dd>
            <dt>…lavorare con Overleaf: si può scrivere direttamente nel mio progetto online?</dt>
            <dd>
              <strong>Non tramite Scriptorium, ed è una scelta voluta.</strong> Overleaf non ha un'API pubblica per scrivere nei progetti: l'unica via sarebbe custodire le tue credenziali (o un token valido su <em>tutti</em> i tuoi progetti) dentro l'app — una responsabilità che non vogliamo prenderci, oltre che contraria alle condizioni d'uso di Overleaf. Le due strade pulite sono:
              <ul>
                <li><strong>Con il ponte Git</strong> (funzione a pagamento di Overleaf): su Overleaf apri Menu → Git e copia l'indirizzo, genera un token dalle impostazioni dell'account, poi dal <strong>Terminale</strong> di Scriptorium fai <code>git clone</code> nella cartella dei progetti e lavora con <code>git pull</code>/<code>git push</code>. Le credenziali restano nel gestore di Windows: Scriptorium non le vede mai. Il pulsante <strong>+ .gitignore</strong> nei Progetti prepara la cartella. Limiti dichiarati da Overleaf: solo il ramo principale, e un push può spostare o perdere commenti e revisioni.</li>
                <li><strong>Senza il ponte</strong> (piano gratuito): su Overleaf Menu → Download → Source per lo <code>.zip</code>, poi «Importa → Da .zip…» nei Progetti; per tornare indietro carichi lo zip su Overleaf. È manuale, ma è un limite di Overleaf, non nostro.</li>
              </ul>
            </dd>
            <dt>…importare la bibliografia di un progetto Overleaf e scaricarne i paper?</dt>
            <dd>Scarica da Overleaf lo <strong>zip del progetto</strong> (Menu → Download → Source) e usa <strong>Importa → «Da bibliografia o progetto…»</strong>: legge il <code>.bib</code> <em>dentro</em> l'archivio (non serve scompattarlo), crea una scheda per ogni voce citata, e — se rispondi sì alle due domande — <strong>cerca e scarica i PDF open-access</strong> (arXiv, Unpaywall, OpenAlex, Semantic Scholar) e mette tutto in una <strong>raccolta</strong> dedicata. Serve la ricerca online attiva. Poi, sulla selezione, «Tag automatici (AI)» per i tag. Alla terza domanda puoi anche far <strong>ricostruire il progetto LaTeX completo</strong> (file .tex, immagini, classi, sottocartelle) dentro <strong>Progetti (LaTeX)</strong>, pronto da compilare. Per importare invece i PDF già compilati che stanno nell'archivio e il grafo delle citazioni del <em>tuo</em> lavoro, usa in aggiunta «Importa → Progetto LaTeX (.zip)…».</dd>
            <dt>…portare la mia libreria da Zotero, Mendeley o EndNote?</dt>
            <dd>Nel gestore fai <strong>Esporta</strong> in <strong>BibTeX/BibLaTeX, RIS o CSL-JSON</strong> (per avere anche i PDF, in Zotero spunta «Esporta file»). Poi barra → Importa → <strong>Da bibliografia o progetto…</strong>, scegli il file e — se i PDF stanno in una cartella a parte — indicala quando te lo chiede. Metadati, PDF e parole chiave (→ tag) entrano insieme, senza doppioni.</dd>
            <dt>…sistemare un paper arrivato senza titolo o con metadati sbagliati?</dt>
            <dd>Clic su «✦ N senza metadati» in alto per il recupero in blocco (solo abbinamenti sicuri). Per il caso singolo: tasto destro → Organizza → <strong>Recupera metadati…</strong> mostra i candidati trovati online con le prove nel PDF e applichi quello giusto (o incolli un DOI/arXiv). Ritocchi a mano: Modifica metadati. Per tutta la libreria: Impostazioni → Manutenzione → «Verifica e ripara metadati».</dd>
            <dt>…organizzare i paper in cartelle e sottocartelle, anche su disco?</dt>
            <dd>Apri l'<strong>Archivio</strong> (icona cartella): crei raccolte e sotto-raccolte e trascini i paper tra i nodi; «<strong>✦ Suggerisci</strong>» propone cosa metterci (con soglia di confidenza). Vuoi vederle anche in Esplora risorse o dal terminale? Attiva lo <strong>Specchio su disco</strong> (chip in alto): una cartella con l'albero delle raccolte e i PDF con nomi leggibili, sempre sincronizzata, senza occupare spazio in più.</dd>
            <dt>…vedere cosa sta facendo l'app in questo momento (e perché qualcosa è fallito)?</dt>
            <dd>Apri la <strong>Plancia</strong> (icona tachimetro sulla barra): un sinottico in finestra separata, da tenere anche in background, dove si illumina solo ciò che sta lavorando. Gli errori accendono il nodo in rosso col motivo per esteso; clic sul nodo per dettagli e storico; «Salva registro…» esporta la sessione. Da Impostazioni → Manutenzione puoi registrare l'attività anche su file.</dd>
            <dt>…copiare una citazione pronta?</dt>
            <dd>Tasto destro sul paper → Cita: APA, IEEE, BibTeX, citekey, <code>\cite</code>, <code>[@…]</code>. Con più paper selezionati ottieni <code>\cite&#123;k1,k2&#125;</code> o tutte le voci BibTeX insieme.</dd>
            <dt>…mandare un paper a un collega?</dt>
            <dd>Tasto destro → Condividi: WhatsApp/Teams/Gmail aprono il messaggio pronto e il PDF è già copiato (incollalo con <kbd>Ctrl</kbd>+<kbd>V</kbd>); Outlook desktop lo allega da solo.</dd>
            <dt>…estrarre una tabella che viene male?</dt>
            <dd>Nella finestra della tabella cambia motore: <strong>Modello</strong> per le tabelle dei paper (anche senza bordi), <strong>Ollama</strong> per le scansioni. Il rettangolo deve coprire tutta la tabella.</dd>
            <dt>…correggere una formula riconosciuta male?</dt>
            <dd>Il LaTeX nella finestra è modificabile e l'anteprima si aggiorna live. Se sono più equazioni impilate prova «Più righe»; in alternativa cambia motore.</dd>
            <dt>…portare un passaggio di un paper nei miei appunti, con la fonte?</dt>
            <dd>Seleziona il testo nel lettore → «Manda agli Appunti» (radiale o barretta): entra come citazione con <code>[[@citekey]]</code> in coda, che diventa un backlink.</dd>
            <dt>…non perdere gli appunti che scrivo su un gruppo di paper?</dt>
            <dd>Agganciali alla raccolta: dal pulsante «Raccolta…» sopra l'editor dell'appunto, o dalla sezione <strong>Appunti</strong> nel pannello dell'Archivio, che ha anche «+ Appunto con i N paper» per crearne uno già agganciato e con i paper elencati. In elenco puoi filtrare per raccolta — «Senza raccolta» ti mostra quelli mai sistemati. Sganciare, o eliminare la raccolta, non tocca il file <code>.md</code>.</dd>
            <dt>…scrivere una formula in un appunto?</dt>
            <dd>Scrivi <code>$$ E = mc^2 $$</code>: la vedi resa in Affiancato/Anteprima e negli export.</dd>
            <dt>…mettere un'immagine in un appunto?</dt>
            <dd>Trascinala o incollala nell'editor: viene salvata come file in <code>assets/</code> e nel testo resta un riferimento breve.</dd>
            <dt>…esportare un appunto in PDF?</dt>
            <dd>Apri l'appunto → pulsante <strong>PDF</strong> → nella finestra di stampa scegli «Salva come PDF». Ci sono anche MD, HTML e LaTeX.</dd>
            <dt>…scrivere un articolo LaTeX citando la mia libreria?</dt>
            <dd>Barra → Progetti (LaTeX) → Crea (scegli un modello). «Cita» inserisce <code>\cite&#123;…&#125;</code> dalla libreria, «Compila» produce il PDF con anteprima. Per un template ufficiale: scaricalo in .zip e usa «Da .zip…».</dd>
            <dt>…compilare se MiKTeX chiede Perl o non ho un compilatore?</dt>
            <dd>Con MiKTeX la compilazione passa da texify e non serve Perl. Se non hai nulla: <code>winget install Tectonic.Tectonic</code> (si scarica i pacchetti da solo).</dd>
            <dt>…sapere quali paper citati mi mancano?</dt>
            <dd>Barra → Cura della libreria → Gap di citazioni. «Risolvi DOI dei riferimenti» ne fa entrare altri nel conteggio.</dd>
            <dt>…seguire un tema e accorgermi dei paper nuovi?</dt>
            <dd>Cerca online → ★ Salva la ricerca. A ogni avvio i risultati nuovi si raccolgono nella campana Novità.</dd>
            <dt>…perché la Costellazione è vuota o mancano i paper nuovi?</dt>
            <dd>Serve l'Indice semantico: l'icona sulla barra lo costruisce/aggiorna. I nuovi documenti (e appunti) entrano al prossimo aggiornamento.</dd>
            <dt>…riaprire l'ultimo PDF al punto in cui ero?</dt>
            <dd>«Riprendi lettura» sulla barra (o radiale/palette). Ogni PDF comunque riparte dall'ultima pagina letta.</dd>
            <dt>…perché le funzioni AI non partono?</dt>
            <dd>Guarda il chip in alto: se dice «AI off», cliccalo. Poi verifica in Impostazioni → AI locale che il server (Ollama/LM Studio) sia raggiungibile e il modello esista.</dd>
            <dt>…liberare la memoria della GPU senza spegnere tutto?</dt>
            <dd>Barra → Memoria AI → «Libera GPU». «Ferma AI» invece chiude anche il server e disattiva l'AI.</dd>
            <dt>…trovare un comando che non ricordo dove sta?</dt>
            <dd><kbd>Ctrl</kbd>+<kbd>K</kbd> e digitalo: trovi azioni, documenti, appunti, pagine wiki, progetti e le sezioni di questa guida — anche mentre leggi un PDF. Funziona pure nel menu radiale: apri e digita per filtrare.</dd>
          </dl>
        </div>
        {/if}
