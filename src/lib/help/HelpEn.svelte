<script lang="ts">
  /* Prosa della guida, versione INGLESE. Gemella di HelpIt.svelte: stessa
     struttura, stesse sezioni, stesso numero di <h3>, <li>, <dt>/<dd> e <tr>.
     Se una scheda perde un pezzo qui, non se ne accorge nessuno guardando
     l'italiano: la parita' strutturale e' il controllo, e va tenuta.

     Trappola: dentro un .svelte una graffa nuda e' un'espressione — le graffe
     letterali di \cite{…} restano scritte &#123; / &#125;, come in italiano.

     i18n-exempt: prosa per lingua, non chiavi di dizionario. */
  import type { HelpTab } from "./tabs";

  let { tab }: { tab: HelpTab } = $props();
</script>

        {#if tab === "inizia"}
        <div class="helpsec">
          <h3>The three ways in</h3>
          <ul>
            <li><strong>Toolbar</strong> (at the top): one icon per tool — hover over it for the name. In order: <strong>My papers</strong> (back to the grid), <strong>Import</strong>, <strong>View</strong>, <strong>Resume reading</strong>, <strong>Ask your library</strong>, <strong>Wiki</strong>, <strong>Search online</strong>, <strong>Notes</strong>, <strong>Projects (LaTeX)</strong>, <strong>Archive</strong> (collections and sub-collections, seen at a glance), <strong>Rediscover</strong>, <strong>New papers</strong> (🔔 with the count of papers you have not seen), <strong>Export</strong>, <strong>Library care</strong>, <strong>Semantic index</strong>, <strong>AI memory</strong> (or <em>Turn the AI on</em> when it is off), <strong>Backup</strong>, <strong>Trash</strong>, <strong>Terminal</strong> (&gt;_), <strong>Bridge</strong> (the process panel, in a window of its own), <strong>Guide</strong>, <strong>Appearance</strong>, <strong>System</strong> (Settings · Check for updates · About). Entries with a menu open on click, the others run straight away; the icon is highlighted when you are in the matching view. Up there you also find the <strong>AI</strong> chip (the state of the local AI), “✦ N without metadata” when it is needed, and the <strong>palette</strong> icon.</li>
            <li><strong>Radial menu</strong> (right-click): on a <strong>document</strong> → the actions for that document; on <strong>empty space</strong> → the global menu (the same groups as the toolbar); on a <strong>multiple selection</strong> → the bulk actions. Move towards a petal and click (the direction is enough); the <strong>wheel</strong> rotates it; <strong>type</strong> to filter every entry at any depth; the hub goes back, <kbd>Esc</kbd> closes. The <strong>description</strong> of the highlighted entry appears under the ring.</li>
            <li><strong>Command palette</strong> (<kbd>Ctrl</kbd>+<kbd>K</kbd>): every action, document, <strong>note</strong>, <strong>wiki page</strong>, <strong>LaTeX project</strong>, filter, section of this guide and theme — just by typing. It works <strong>inside the reader too</strong>. Toolbar, radial menu and palette all draw on the <strong>same register</strong>: if you cannot find a command, it is in there anyway.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>The grid and the sidebar</h3>
          <ul>
            <li><strong>One click</strong> on a card opens the <strong>detail panel</strong> on the right (abstract, AI summary, editable tags, citations, notes); <strong>double click</strong> or <kbd>Enter</kbd> open the reader.</li>
            <li>The <strong>sidebar</strong> is the navigation: quick filters (Favorites, Unread, With code, Peer-reviewed, My own work), tags, collections, saved searches, watched folder. <kbd>Ctrl</kbd>+<kbd>B</kbd> shows and hides it.</li>
            <li>In the “All” view: the <strong>Overview</strong> (unread, being read, added this month + one paper to rediscover each day) and <strong>Keep reading</strong> (the PDFs you opened last). In the reader the bar fades away while you read, and the zoom is remembered per document.</li>
            <li><strong>This guide is a window</strong>: drag it by its title bar, resize it from the bottom-right corner (like nearly every dialog here) and tick <strong>“always on top”</strong> to keep it in sight — over the reader as well — while you follow the steps.</li>
          </ul>
          <table class="kbdtable">
            <tbody>
              <tr><td><kbd>Ctrl</kbd>+<kbd>K</kbd></td><td>Command palette — everything, by typing</td></tr>
              <tr><td><kbd>/</kbd></td><td>Search the library</td></tr>
              <tr><td><kbd>Ctrl</kbd>+<kbd>B</kbd></td><td>Show/hide the sidebar</td></tr>
              <tr><td><kbd>arrows</kbd> / <kbd>Space</kbd></td><td>Move between cards / open and close the panel</td></tr>
              <tr><td><kbd>X</kbd> / <kbd>F</kbd></td><td>Select / favorite</td></tr>
              <tr><td><kbd>Enter</kbd> / <kbd>Esc</kbd></td><td>Open the reader / close</td></tr>
              <tr><td>Right-click</td><td>Radial menu (document, selection or global)</td></tr>
              <tr><td><kbd>Tab</kbd> → <kbd>Enter</kbd></td><td>Reach the toolbar and open its menu</td></tr>
              <tr><td><kbd>↑</kbd> <kbd>↓</kbd> inside menus</td><td>Run through the entries — <kbd>Esc</kbd> closes and returns to the icon</td></tr>
            </tbody>
          </table>
          <p class="dimtext">Everything is reachable from the keyboard: in the <strong>Archive</strong> the collection tree takes focus with <kbd>Tab</kbd>, then <kbd>↑</kbd><kbd>↓</kbd> run through the list, <kbd>→</kbd><kbd>←</kbd> enter and leave the sub-collections, and <kbd>Enter</kbd> opens the collection in the grid.</p>
        </div>

        {:else if tab === "libreria"}
        <div class="helpsec">
          <h3>Importing</h3>
          <ul>
            <li><strong>Six routes</strong> (toolbar → Import): <strong>PDFs from disk</strong> (dropping them into the window works too — they stay where they are, the app indexes them; duplicates are recognized by content); <strong>From a bibliography or a project</strong> — the export of <strong>Zotero, Mendeley, EndNote, JabRef…</strong> in <strong>.bib / .ris / CSL-JSON</strong>: it brings the metadata over, attaches the PDFs (from the <em>file</em> field, or from an export folder you point at) and turns the keywords into <strong>tags</strong>, with no duplicates (dedup by DOI and by PDF content; works already in the Trash come back into view); by <strong>identifier</strong> (DOI / arXiv / ISBN / PMID); <strong>from a URL</strong>; <strong>LaTeX project (.zip)</strong> — your own papers with their bibliography, flagged “My own work”; <strong>Watched folder</strong> (whatever lands in it is imported on its own).</li>
            <li><strong>From the browser</strong>: copy the link to the PDF and come back to Scriptorium — a “Grab it” prompt appears (switch in Settings → Browser connector); failing that, the <strong>bookmarklet</strong>, or the Watched folder pointed at Downloads.</li>
            <li><strong>References with no PDF</strong> (added from an online search, from citations, from BibTeX or by ID): <strong>Find PDF…</strong> (the card's radial menu, or by opening the entry) shows the <strong>candidates</strong> found online by identifier and by title (arXiv, Unpaywall, OpenAlex, Semantic Scholar, Crossref) together with the evidence — “Download and attach” the right one, “Open page” to check, or paste a direct link. On a <strong>multiple selection</strong> and in bulk (Library care → “Find PDFs for the references”) it stays automatic: it only attaches safe matches, now by title on arXiv/S2 as well.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Organizing</h3>
          <ul>
            <li>Colored <strong>tags</strong> (the <strong>✎</strong> in the sidebar renames and recolors, the <strong>×</strong> deletes; from the detail panel you apply them on the fly) and <strong>Collections</strong>, <em>smart</em> ones included (they fill themselves from a rule).</li>
            <li><strong>Filters</strong> in the sidebar (Favorites, Unread, With code, Peer-reviewed, My own work), <strong>sorting you can stack</strong> (the “Sort ▾” chip: one click turns a criterion on, another reverses it, a third drops it), <em>preprint / peer-reviewed</em> badges on the cards.</li>
            <li><strong>Views</strong> (toolbar → View): grid (covers resized with − ▭ +), list with columns, <strong>Constellation</strong> (the semantic map — see the <em>Discovery</em> tab). Click an <strong>author</strong> → everything they wrote.</li>
            <li><strong>Archive</strong> (the folder icon on the toolbar): your collections as a <strong>tree you can walk</strong> — sub-collections as deep as you like, <strong>drag a paper</strong> onto a collection to move it there (Ctrl = add it there as well: a paper can belong to several; <strong>onto the empty background</strong> = take it out of the collection), drag a collection onto another to nest it. Deleting a collection never touches the papers (its sub-collections move up one level). You can open the <strong>Constellation of a single collection</strong> in three ways: the <strong>✳</strong> button next to the collection's name in the sidebar, the <em>Constellation of “name”</em> entry in the <strong>View</strong> menu while you are looking at it, or the <strong>CONSTELLATION</strong> button in the Archive panel. Proximity is recomputed inside it, and the layout is saved apart from the general map. In the panel: <strong>✦ Suggest</strong> proposes the papers that belong (local semantic similarity, with a confidence threshold — never automatic): pick the source <em>before</em> computing — the collection's <em>Name</em> (this works with the engines off too: it anchors to your papers that contain those words), <em>Content</em> (the papers already in it) or <em>Both</em>, with an <strong>adjustable weight</strong>. The <strong>“New papers” search</strong> toggle attaches an online search to the collection (the items you accept <em>join the collection on their own</em>, filtered for relevance once the collection holds ≥3 indexed papers; switching it off removes the search).</li>
            <li>Every collection's panel also has a <strong>Notes</strong> section: the <code>.md</code> notes linked to that collection, plus <strong>“+ Note with its N papers”</strong>, which creates one already linked and with the papers listed as citations. It exists so you do not end up with a flat list of syntheses that get lost; the details are in the <em>Writing</em> tab.</li>
            <li><strong>Disk Mirror</strong> (the chip at the top of the Archive): it projects your collections into a real folder — <code>Collection\Subcollection\Author Year — Title.pdf</code> — using <strong>hardlinks</strong> (no extra space), kept up to date on its own at every change. Very handy from File Explorer and from the terminal. Deleting or moving files inside the mirror does not touch the library (it is rebuilt); <em>editing the contents</em> of a PDF in there does, because it is the same file: use the reader to annotate.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Sharing and printing</h3>
          <ul>
            <li>Right-click → <strong>Share</strong>: <strong>WhatsApp / Teams / Gmail</strong> open the draft with the message ready and the <strong>PDF already copied ✓ to the system clipboard</strong> — paste it with <kbd>Ctrl</kbd>+<kbd>V</kbd>; <strong>Outlook desktop</strong> attaches the file itself. It works on a <strong>multiple selection</strong> too.</li>
            <li><strong>Print</strong>: from the reader (the <strong>⋯ More</strong> menu or the radial menu) for the open document, or from the selection's radial menu to print several at once.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Metadata</h3>
          <ul>
            <li>When some records are incomplete, “<strong>✦ N without metadata</strong>” appears: one click fetches them in bulk (with a progress bar and a Stop button) — first the arXiv id in the <strong>file name</strong> (this works on scans as well), then the DOI and the <strong>title</strong> from the PDF (Crossref/arXiv). Never from the first DOI found in the text: no wrong labels — whatever is not certain is left as it is.</li>
            <li>For the single, stubborn case: right-click → Organize → <strong>Fetch metadata…</strong> runs the <em>extended</em> search (Crossref, arXiv, OpenAlex, every DOI/arXiv id printed in the PDF, the file name) and shows the <strong>candidates with the evidence</strong> found in the PDF (title, authors, year): you choose which one to apply, or paste a DOI/arXiv id. Same thing from Library health (“Find…”) next to every incomplete document.</li>
            <li><strong>Settings → Maintenance → “Check and repair metadata”</strong> goes over the whole library and fixes the records whose title does not match the PDF. Safe and repeatable. By hand: right-click → <strong>Edit metadata</strong>.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Care, backup, trash</h3>
          <ul>
            <li><strong>Library care</strong> (toolbar): <strong>Health</strong> (missing files, PDFs with no text → OCR, incomplete metadata), <strong>Citation gaps</strong> (the DOIs your papers cite most and you do not own; “Resolve reference DOIs” fetches more of them online, never on an uncertain match), <strong>Duplicates</strong> (merging), <strong>Rebuild previews</strong>.</li>
            <li><strong>Backup library</strong> (toolbar): a complete copy — PDFs and database — into a folder of your choice. <strong>Trash</strong>: what you delete stays restorable until you empty it.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Searching the library</h3>
          <ul>
            <li>The search bar at the top (<kbd>/</kbd>), in three modes: <em>All</em> (text + meaning), <em>Text</em> or <em>Semantic</em> (by meaning — it needs the <strong>Semantic index</strong>). It searches your <strong>annotations</strong> and your <strong>document notes</strong> as well; matching <strong>.md notes</strong> appear in a group of their own above the results.</li>
          </ul>
        </div>

        {:else if tab === "scoperta"}
        <div class="helpsec">
          <h3>Search online</h3>
          <ul>
            <li>Toolbar → <strong>Search online</strong>: arXiv, OpenAlex, ADS, Semantic Scholar, Europe PMC, CORE, DOAJ, <strong>HF Papers</strong> (the index that carries the papers' GitHub repos). Year/author/OA-only filters, <strong>With code</strong> / <strong>Peer-reviewed</strong> / <strong>Preprint</strong> chips, sortable columns. Open Access PDFs are <strong>downloaded</strong>, the others come in as a <strong>reference</strong> (you attach the PDF later).</li>
            <li><strong>Saved searches → New papers</strong>: after a search press <em>★ Save</em> → it goes into the sidebar. Results that are <em>new</em> since last time collect in the <strong>New papers</strong> bell (🔔, re-checked at every start-up): from there <em>Add</em> or <em>Dismiss</em>.</li>
            <li><strong>Rediscover</strong> (toolbar): it fishes out a document you forgot, or never read.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Code, citations and repositories</h3>
          <ul>
            <li>Papers that cite a repo show the <strong>GitHub</strong> icon (card, list, online results): click it to open the repository.</li>
            <li>Right-click → <strong>Code &amp; repos</strong>: a preview of the <strong>README</strong> inside the app, plus the models and datasets linked on <strong>Hugging Face</strong>.</li>
            <li>The <strong>“With code (GitHub)”</strong> filter in the sidebar shows only the papers whose code is available.</li>
            <li>Right-click → Cite → <strong>References and citations</strong>: the paper's bibliography (with the references already in your library clickable) and the documents that <strong>cite</strong> it. That is also where <strong>Copy APA / IEEE / BibTeX / citekey / \cite / [@…]</strong> live. With several papers <strong>selected</strong>, right-click → Cite copies a single <code>\cite&#123;k1,k2&#125;</code> or every BibTeX entry at once (for LaTeX/Pandoc).</li>
            <li><strong>Explore citations (online)</strong> opens on the <strong>Map</strong>: references on the left (the past it stands on), citations on the right (the future), in year order; a <strong>solid</strong> dot = already in the library, <strong>dashed</strong> = missing, a bigger node = a more cited paper. Click a node → add / open / <em>explore from here</em> (the map recenters, “← Back” returns). The <strong>List</strong>, with every action (+ PDF, save), is one click away.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Constellation — the map of your library</h3>
          <ul>
            <li>Toolbar → View → <strong>Constellation</strong>: every <strong>star</strong> is a document, the <strong>purple diamonds</strong> are your notes, and the links are <strong>similarity of meaning</strong>. It needs the <strong>Semantic index</strong>. If some papers have no embedding yet, <strong>“✦ N off the map”</strong> appears at the top: press it and the index is generated <em>from there</em>, with the map redrawing itself when the job finishes — no need to leave and come back. The <strong>↻</strong> button reloads the graph (handy after changing its density), and the map refreshes on its own anyway when you import or delete documents. Positions are remembered between sessions.</li>
            <li><strong>Click</strong> = the node's card (with its links as percentages, clickable); <strong>double click</strong> = open; <strong>right-click</strong> = menu; <strong>Ctrl+click</strong> = select. From far away you see the <strong>nebulae</strong> — the semantic communities with their label; up close the <strong>✓</strong> badges (peer-reviewed) and the fork (GitHub code) show up.</li>
            <li>Top right: <strong>Search the graph</strong> (2-3 letters of a title or of an author are enough → suggested candidates; Enter or a click lights the star with a pulsing halo and centers the view on it; × or Esc clears it), <strong>Color by</strong> (Dominant tag / Semantic communities / Year / Read status), <strong>Nebulae</strong> and the <strong>⚙</strong> panel (Links per node, Similarity threshold).</li>
            <li><strong>Explore the surroundings (online)</strong>, in a star's card: <em>Citations</em> / <em>Similar</em> / <em>Author</em> → the results appear as dashed <strong>ghost stars</strong> around it; click → “Add to the library”. They join the graph at the next index update. Every ghost carries the same two marks as the real stars — a <strong>green ✓</strong> if peer-reviewed, <strong>⑂</strong> if it has a code repository — so you can decide what to add without opening one card at a time. The <strong>✓⑂</strong> button at the top <em>brings those out</em>: the others fade, none disappear.</li>
            <li><strong>Exploration chain</strong>: a <em>ghost star</em>'s card carries Citations / Similar / Author too — new finds hook onto that one, in a chain (snowball), so you dig from paper to paper without adding anything until you hit the right one. While you explore, the map enters <strong>exploration mode</strong>: the library dims, the seeds wear a “scanner” ring, each generation of the chain has its own color and the links flow with an animation; chains lay themselves out without overlapping. <strong>Citations</strong> works without a DOI as well (OpenAlex ids for the finds; strict title matching for your own papers).</li>
          </ul>
        </div>

        {:else if tab === "lettura"}
        <div class="helpsec">
          <h3>The reader</h3>
          <ul>
            <li><strong>Immersive</strong>: the bar fades away while you read (the mouse brings it back), the rarer tools sit under <strong>⋯ More</strong>, the zoom is remembered per document and you start again from the <strong>last page</strong>. “<strong>Resume reading</strong>” (toolbar/radial menu/palette) reopens the last PDF where you left off.</li>
            <li><strong>Annotations</strong> (<kbd>A</kbd>): highlights with a color and a comment, or <strong>sticky notes</strong> pinned to one spot on the page. <strong>Document note</strong> (<kbd>E</kbd>): a free-form note about the <em>whole</em> paper. <strong>.md notes</strong>, on the other hand, are independent files (see the <em>Writing</em> tab).</li>
            <li><strong>Finding a highlight when you cannot remember which paper it was in</strong>: the search bar at the top looks <em>inside</em> your highlights and comments as well — the results appear in the “<strong>Highlights</strong>” group, and one click opens the PDF at the exact page. The <strong>✎ n</strong> mark on the card says how many it holds, without opening it.</li>
            <li><strong>Moved or renamed a PDF outside Scriptorium?</strong> When you open it you get “<strong>Find the file…</strong>”: you say where it is now and the record survives intact (highlights, tags, note, citekey). If you moved a <em>whole folder</em>, after the first one it offers to sort out all the others in one go, checking each file's fingerprint. The same button lives in <em>Library care → Files missing from disk</em>.</li>
            <li><strong>Find in the document</strong> (<kbd>Ctrl</kbd>+<kbd>F</kbd>) in a band of its own; <kbd>Enter</kbd>/<kbd>Shift</kbd>+<kbd>Enter</kbd> step through the results. Plus the outline, zoom/fit, rotation, two-page view, night mode.</li>
            <li><strong>AI Lens</strong>: select a passage → <em>Explain</em> / <em>Translate</em> / <em>Ask</em> (with the local AI on); the answer can be saved into the Document note.</li>
            <li><strong>Send to Notes</strong>: a selection → the radial menu or the highlight's little bar → pick the note → it arrives as a quotation with the <code>[[@citekey]]</code> reference at the end. The <strong>numbered sources</strong> of “Ask your library” open the PDF at the right page.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Extracting from the PDF: tables, text, formulas, figures</h3>
          <ul>
            <li>Press the key for the mode (<kbd>T</kbd> table, <kbd>X</kbd> text, <kbd>F</kbd> formula, <kbd>G</kbd> figure — or the icons/⋯) and <strong>drag a rectangle</strong> over the area. The window that opens can be dragged and resized; everywhere you get <strong>Copy / Save / → Notes</strong> with the <strong>format of your choice</strong> (the preview shows exactly what comes out). Whatever you send to Notes carries the <code>[[@citekey]]</code> reference with it.</li>
            <li><strong>Table</strong> (<kbd>T</kbd>) — three engines in the header: <strong>Native</strong> (from the PDF's text, fast, for simple tables), <strong>Model</strong> (it reads rows/columns/headers off the image and takes the <em>exact</em> text from the PDF — the best one on papers; ~111 MB downloaded on first use; it does not work on rotated pages), <strong>Ollama</strong> (a vision model, for scans). Export: grid, CSV, Markdown, Excel, LaTeX (booktabs), plus “Improve with AI”.</li>
            <li><strong>Text</strong> (<kbd>X</kbd>) — <strong>Native</strong> keeps the <strong>formatting</strong> (italics, bold, super/subscripts → Markdown, e.g. H<sub>2</sub>O); <strong>Ollama</strong> does the OCR of scanned pages. Export: plain text, Markdown, LaTeX.</li>
            <li><strong>Formula</strong> (<kbd>F</kbd>) — the <strong>Local</strong> engine (built-in math-OCR, ~115 MB on first use) or <strong>Ollama</strong>; “<strong>Multi-line</strong>” for stacked equations. The recognized LaTeX is <strong>editable</strong>, with the rendering updated as you type; the <code>\mathrm&#123;&#125;</code> button sets the whole thing in upright type. Export: LaTeX or <code>$$…$$</code>.</li>
            <li><strong>Figure</strong> (<kbd>G</kbd>) — crop any area as an image: <strong>Save PNG…</strong> or <strong>→ Notes</strong>.</li>
          </ul>
          <table class="kbdtable">
            <tbody>
              <tr><td><kbd>Ctrl</kbd>+<kbd>F</kbd></td><td>Find in the document</td></tr>
              <tr><td><kbd>+</kbd> / <kbd>−</kbd> / <kbd>0</kbd></td><td>Zoom in / out / 100%</td></tr>
              <tr><td><kbd>W</kbd> / <kbd>H</kbd></td><td>Fit to width / to page</td></tr>
              <tr><td><kbd>2</kbd></td><td>Two-page view</td></tr>
              <tr><td><kbd>N</kbd></td><td>Add a sticky note</td></tr>
              <tr><td><kbd>A</kbd> / <kbd>E</kbd></td><td>Annotations panel / Document note</td></tr>
              <tr><td><kbd>T</kbd> / <kbd>X</kbd> / <kbd>F</kbd> / <kbd>G</kbd></td><td>Extract table / text / formula / figure (press again to cancel)</td></tr>
              <tr><td><kbd>I</kbd></td><td>Night mode (invert the colors)</td></tr>
              <tr><td><kbd>[</kbd> / <kbd>]</kbd></td><td>Rotate left / right</td></tr>
              <tr><td><kbd>Ctrl</kbd>+wheel</td><td>Continuous zoom</td></tr>
              <tr><td><kbd>Esc</kbd></td><td>Close / cancel</td></tr>
              <tr><td><kbd>?</kbd></td><td>Shortcuts (inside the reader)</td></tr>
            </tbody>
          </table>
        </div>

        {:else if tab === "scrittura"}
        <div class="helpsec">
          <h3>Library wiki</h3>
          <ul>
            <li>Your <strong>private encyclopedia</strong>: one page per concept, written by the local AI reading nothing but your own documents (the <strong>Wiki</strong> icon on the toolbar).</li>
            <li>The sources, in three ways: <strong>“Generate/update from tags”</strong> (one page for every tag with at least 2 documents); a <strong>free concept</strong> (semantic search finds the papers that fit); or <strong>choose the sources yourself</strong> — select the documents in the grid → right-click → <em>Wiki page (AI)</em>: the page uses exactly those (10 at most).</li>
            <li>The <strong>[n]</strong> citations in the text open the PDF <strong>at the right page</strong>; concepts mentioned in other pages turn into <strong>links</strong> between pages; at the bottom you find the sources with the passages that were used (“p. N” chips).</li>
            <li>Every source <em>has</em> to appear in the page: if the synthesis does not use one, it is declared under “sources not integrated” — never dropped in silence. The <strong>●</strong> dot in the list warns you that the library has changed and the page is worth regenerating.</li>
            <li>It needs the local AI on and the passage index (Ask your library → Build the index). A tip: a model ≥ 8B (e.g. <code>gemma3:27b</code>) for prose that holds up.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Notes</h3>
          <ul>
            <li><strong>Real .md files</strong> in the app's folder: you can open and edit them from an outside editor or from the terminal (the index realigns at every start-up). The <strong>Notes</strong> icon on the toolbar; the list sorts by modification/creation/title. <em>Not the same</em> as Annotations (anchored to the PDF) or the Document note (one per paper).</li>
            <li><strong>[[Links]]</strong>: <code>[[Title of a note]]</code> links a note; <code>[[@citekey]]</code> or <code>[[Title of a paper]]</code> link a document (click → it opens). <strong>Backlinks</strong> at the bottom. All of it searchable from the search bar.</li>
            <li>An <strong>editor</strong> with autosave and three modes: <em>Edit</em>, <em>Side by side</em> (live preview), <em>Preview</em>. A formatting bar (headings, bold, lists, move blocks), <strong>formulas</strong> with <code>$$ … $$</code> (rendered in the preview), <strong>images</strong> dropped or pasted (saved as files in <code>assets/</code>, only a short reference stays in the text). <strong>Rename</strong> (or a double click on the title) renames the file as well.</li>
            <li><strong>Linking a note to a collection</strong> — so a note discussing a group of papers lives <em>inside</em> that group, instead of at the bottom of a list that grows with every synthesis. Two ways in: the <strong>“Collection…”</strong> button above the editor (for a note you have already written), or the <strong>Archive</strong>, where every collection's panel now has a <strong>Notes</strong> section. A note can belong to several collections. In the list the collection shows as a tag, and the <strong>Collection</strong> menu filters — including “No collection”, to find the ones you never filed.</li>
            <li>From the Archive, <strong>“+ Note with its N papers”</strong> creates the note <em>already linked</em>, with the collection's papers listed as <code>[[@citekey]]</code> citations: citekeys are not on screen — they live in each paper's card — so the list is the tedious part you are spared. The list only: no structure is imposed, because that part is yours. Unlinking a note, or deleting the collection, <strong>never touches the .md file</strong>.</li>
            <li><strong>Export</strong> (under the title): <strong>MD</strong> · <strong>HTML</strong> (a standalone page) · <strong>LaTeX</strong> (.tex + figures in a folder) · <strong>PDF</strong> — formulas and figures included.</li>
            <li>Notes <strong>receive</strong> from the reader: selected text, extracted tables, formulas and figures, always with the <code>[[@citekey]]</code> reference; from the detail panel the Abstract and the AI summary too; <strong>AI reviews</strong> can be saved as a note. With the Semantic index on, notes also show up in the <strong>Constellation</strong> (the purple diamonds).</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Projects (LaTeX) — a small local Overleaf</h3>
          <ul>
            <li>The <strong>Projects (LaTeX)</strong> icon on the toolbar. Every project is a <strong>real folder</strong> (under <code>projects/</code> in the app's data) with <code>main.tex</code> and <code>refs.bib</code>. Create one from <strong>5 templates</strong> (article, two-column paper, report/thesis, beamer presentation, minimal) or from “<strong>From .zip…</strong>” with a template you downloaded — the links to <strong>Overleaf / IEEE / ACM / Springer / Elsevier</strong> are right below.</li>
            <li><strong>Cite</strong>: searches your library and inserts <code>\cite&#123;citekey&#125;</code> at the cursor. <strong>Sync bibliography</strong>: rewrites <code>refs.bib</code> with the whole library. Autosave (or <kbd>Ctrl</kbd>+<kbd>S</kbd>).</li>
            <li><strong>Compile</strong> uses the system compiler: <strong>Tectonic</strong>, or <strong>MiKTeX</strong> (through texify, no Perl needed), or latexmk. The PDF preview appears beside the editor; if the PDF comes out with warnings you still get it, with the log one click away. No compiler at all: <code>winget install Tectonic.Tectonic</code>.</li>
                      <li><strong>Bringing an Overleaf project here</strong>: “From .zip (Overleaf too)…” rebuilds the whole project — .tex, images, classes, subfolders — ready to compile; the third question of the “From a bibliography or a project…” import does the same.</li>
            <li><strong>Syncing with Overleaf</strong>: with Overleaf's Git bridge (a paid feature) you can <code>git clone</code> the project folder from the built-in <strong>Terminal</strong> and work with <code>git pull</code>/<code>git push</code> — the credentials stay in the Windows credential manager, never inside Scriptorium. The <strong>+ .gitignore</strong> button prepares the folder by leaving the build products out. Without the bridge, you go through the <code>.zip</code> (see the Overleaf FAQ).</li>
</ul>
        </div>

        <div class="helpsec">
          <h3>Synthesis tools on a selection</h3>
          <ul>
            <li><strong>Compare (AI)</strong>: select 2-3 papers → right-click → <em>Compare</em>: a table of goal/method/data/results/limits, plus what each one adds.</li>
            <li><strong>Review (AI)</strong>: <strong>select between 2 and 10 papers</strong> → right-click → <em>Review (AI)</em>. You get a <strong>short related-work section</strong> (300-500 words) <strong>organized by theme</strong> (not paper by paper): it compares the approaches, brings out the disagreements and closes with <em>“Open gaps”</em>. Every paper is cited with <strong>clickable [n]</strong> markers (they open the source) and no source is dropped in silence (the unused ones are listed at the end). You can copy it ready for <strong>LaTeX</strong> (<code>\cite&#123;citekey&#125;</code>) or <strong>Pandoc</strong> (<code>[@citekey]</code>), or <strong>“Save to Notes”</strong>: it becomes an <strong>.md note</strong> where the <em>[n]</em> markers are rewritten as <strong><code>[[@citekey]]</code> backlinks</strong> — so every paper cited points back to the review, and it is searchable and editable like any other note. It needs the local AI on.</li>
            <li><strong>Results table (AI)</strong>: it gathers the numbers (method · dataset · metric · value) from the selected papers into one table, exportable to CSV/Markdown/Excel. The values are taken verbatim: always check them against the PDF.</li>
            <li><strong>Reading path</strong> (right-click → AI): to understand a paper, what to read first — the foundations it cites (already yours), the earlier neighbours by content, and the missing references you can add with one click. It works <em>without</em> an LLM.</li>
          </ul>
        </div>

        {:else if tab === "ai"}
        <div class="helpsec">
          <h3>Local AI (optional, never automatic)</h3>
          <ul>
            <li>You need <strong>Ollama</strong> or <strong>LM Studio</strong> on your own PC: set the URL and the model under <strong>Settings → Local AI</strong>. The <strong>AI chip</strong> at the top tells you the state: if it reads “AI off”, one click turns it back on. A tip: a model ≥ 8B (e.g. <code>gemma3:27b</code>).</li>
            <li>What it unlocks: <strong>summaries</strong> and <strong>automatic tags</strong>, the <strong>AI Lens</strong> in the reader, <strong>Ask your library</strong>, the <strong>Wiki</strong>, <strong>Compare/Review</strong>, the <strong>Ollama</strong> engines for tables/text/formulas, “Improve with AI” on tables. Cards with a summary show <strong>✦ AI</strong>; the batch on a selection <strong>skips</strong> whatever already has a summary or tags.</li>
            <li><strong>Ask your library</strong> (toolbar): ask a question in plain language → an answer with <strong>numbered sources</strong> that open the PDFs at the right page. On your first visit press “<strong>Build the index</strong>” (the passage index, reused by the Wiki as well); rebuild it when the library changes a lot.</li>
            <li><strong>AI memory</strong> (toolbar): “<strong>Free GPU</strong>” unloads the models from VRAM and leaves everything enabled (they come back when needed); “<strong>Stop AI</strong>” unloads the models, actually shuts the local server down and disables the AI.</li>
            <li>The <strong>Semantic index</strong> (toolbar) is not generative AI: it is the index of meanings, entirely local. It powers <em>Semantic</em> search, <em>Related</em>, the <strong>Constellation</strong> and the notes in the graph. Run it again after adding documents or notes.</li>
          </ul>
        </div>

        <div class="helpsec">
          <h3>Your data — all of it local</h3>
          <ul>
            <li>The app's folder: <code>%APPDATA%\com.pdfmanage.app</code> — <code>pdfmanage.db</code> (the catalogue), <code>papers/</code> (the PDFs it downloaded; the ones you imported from disk <em>stay where they are</em>), <code>notes/</code> (the .md notes + <code>assets/</code>), <code>projects/</code> (the LaTeX projects), <code>thumbnails/</code> and the local models (<code>mathocr</code>, <code>tablestruct</code>, <code>fastembed_cache</code>).</li>
            <li>Notes and projects are <strong>real files</strong>: editing them from outside is expected. <strong>Backup library</strong> (toolbar) makes a complete copy; <strong>Export</strong> produces citations (BibTeX/RIS/CSL) or notes for <strong>Obsidian</strong>.</li>
            <li><strong>Updates — only when you ask for them.</strong> The app never contacts GitHub on its own: the only check starts from <em>System → Check for updates</em>. If there is one I show you <strong>what changes</strong> and you decide: “Download and install” shows the progress, installs (one small window, no questions) and asks whether to restart. Every update is <strong>signed</strong>: one that is not signed with Scriptorium's key is refused. Your library is not touched — it lives in <code>%APPDATA%</code>, separate from the program — and before any change of version the database is saved to <code>backups/</code> anyway. After the restart the panel with <strong>what's new</strong> appears once.</li>
            <li>The built-in <strong>Terminal</strong> (&gt;_, for <code>claude code</code> among other things); the <code>scriptorium-cli</code> CLI queries from outside, read-only, the library <em>and</em> the Notes <em>and</em> the LaTeX projects (<code>query</code>, <code>bib</code>, <code>notes</code>, <code>note</code>, <code>search-notes</code>, <code>projects</code>, <code>stats</code>…). The <strong>MCP server</strong> <code>scriptorium-mcp</code> carries the same data (9 tools, read-only) into <strong>Claude Desktop / Claude Code</strong> and any MCP client: a ready-made config to copy from <strong>Settings → CLI and MCP</strong>. The <strong>browser connector</strong> behind “Grab it” is a local-only service, switchable off in Settings. <strong>11 themes</strong> under Appearance.</li>
          </ul>

          <h3>Bridge — the process panel</h3>
          <ul>
            <li>The <strong>Bridge</strong> (the gauge icon on the toolbar, or Ctrl+K → “Bridge”) opens a <strong>separate window</strong> with a <strong>visual panel</strong> of the internal processes: imports, extraction, metadata, indexes, AI, backup… You can <strong>keep it in the background</strong> at one side of the screen: <em>only</em> what is really working lights up, with progress (<code>12/96</code>) and duration; idle, it stays dark.</li>
            <li><strong>State and errors at a glance</strong>: a failure turns the node red with the <strong>reason</strong> spelled out; a problem that does not block anything (one file in a hundred, say) is marked amber without stopping the rest. Subsystems that are off say <em>why</em> (“online disabled”, “models to download”…). Click a node: description, statistics, history.</li>
            <li>The <strong>activity log</strong> at the bottom can be filtered (All/Errors) and exported with <strong>Save log…</strong>; from <strong>Settings → Maintenance</strong> you can have it written <strong>to a file</strong> as well (one per day, kept for 14) to work out afterwards what happened.</li>
          </ul>
        </div>

        {:else if tab === "faq"}
        <div class="helpsec">
          <h3>How do I…?</h3>
          <dl class="faq">
            <dt>…add the PDF I have just downloaded in the browser?</dt>
            <dd>Copy the link to the PDF and come back to Scriptorium: a “Grab it” prompt appears. Or point the Watched folder at Downloads: it comes in by itself.</dd>
            <dt>…work with Overleaf — can I write straight into my online project?</dt>
            <dd>
              <strong>Not through Scriptorium, and that is deliberate.</strong> Overleaf has no public API for writing into projects: the only way would be to keep your credentials (or a token valid on <em>all</em> your projects) inside the app — a responsibility we do not want to take on, and one that goes against Overleaf's terms of use. The two clean routes are:
              <ul>
                <li><strong>With the Git bridge</strong> (a paid Overleaf feature): in Overleaf open Menu → Git and copy the address, generate a token from your account settings, then from Scriptorium's <strong>Terminal</strong> run <code>git clone</code> into the projects folder and work with <code>git pull</code>/<code>git push</code>. The credentials stay in the Windows credential manager: Scriptorium never sees them. The <strong>+ .gitignore</strong> button in Projects prepares the folder. Limits Overleaf states itself: the main branch only, and a push can move or lose comments and tracked changes.</li>
                <li><strong>Without the bridge</strong> (free plan): in Overleaf, Menu → Download → Source for the <code>.zip</code>, then “Import → From .zip…” in Projects; to go the other way you upload the zip to Overleaf. It is manual, but that is an Overleaf limit, not ours.</li>
              </ul>
            </dd>
            <dt>…import the bibliography of an Overleaf project and download its papers?</dt>
            <dd>Download the <strong>project zip</strong> from Overleaf (Menu → Download → Source) and use <strong>Import → “From a bibliography or a project…”</strong>: it reads the <code>.bib</code> <em>inside</em> the archive (no need to unpack it), creates a record for every entry cited, and — if you say yes to the two questions — <strong>looks for and downloads the open-access PDFs</strong> (arXiv, Unpaywall, OpenAlex, Semantic Scholar) and puts the lot into a dedicated <strong>collection</strong>. Online search has to be on. Then, on the selection, “Auto-tag (AI)” for the tags. At the third question you can also have the <strong>whole LaTeX project rebuilt</strong> (.tex files, images, classes, subfolders) inside <strong>Projects (LaTeX)</strong>, ready to compile. To import instead the compiled PDFs sitting in the archive and the citation graph of <em>your own</em> work, use “Import → LaTeX project (.zip)…” on top of that.</dd>
            <dt>…move my library over from Zotero, Mendeley or EndNote?</dt>
            <dd>In your manager, <strong>Export</strong> to <strong>BibTeX/BibLaTeX, RIS or CSL-JSON</strong> (to get the PDFs as well, tick “Export files” in Zotero). Then toolbar → Import → <strong>From a bibliography or a project…</strong>, pick the file and — if the PDFs sit in a folder of their own — point at it when you are asked. Metadata, PDFs and keywords (→ tags) come in together, with no duplicates.</dd>
            <dt>…sort out a paper that arrived with no title, or with the wrong metadata?</dt>
            <dd>Click “✦ N without metadata” at the top for the bulk fetch (safe matches only). For a single case: right-click → Organize → <strong>Fetch metadata…</strong> shows the candidates found online with the evidence in the PDF, and you apply the right one (or paste a DOI/arXiv id). Touch-ups by hand: Edit metadata. For the whole library: Settings → Maintenance → “Check and repair metadata”.</dd>
            <dt>…organize papers into folders and subfolders, on disk as well?</dt>
            <dd>Open the <strong>Archive</strong> (the folder icon): you create collections and sub-collections and drag papers between the nodes; “<strong>✦ Suggest</strong>” proposes what to put in them (with a confidence threshold). Want to see them in File Explorer or from the terminal too? Turn on the <strong>Disk Mirror</strong> (the chip at the top): a folder with the tree of your collections and the PDFs under readable names, always in sync, taking no extra space.</dd>
            <dt>…see what the app is doing right now (and why something failed)?</dt>
            <dd>Open the <strong>Bridge</strong> (the gauge icon on the toolbar): a panel in a separate window, meant to be kept in the background, where only what is working lights up. Errors turn the node red with the reason spelled out; click the node for details and history; “Save log…” exports the session. From Settings → Maintenance you can record the activity to a file as well.</dd>
            <dt>…copy a ready-made citation?</dt>
            <dd>Right-click the paper → Cite: APA, IEEE, BibTeX, citekey, <code>\cite</code>, <code>[@…]</code>. With several papers selected you get <code>\cite&#123;k1,k2&#125;</code> or every BibTeX entry at once.</dd>
            <dt>…send a paper to a colleague?</dt>
            <dd>Right-click → Share: WhatsApp/Teams/Gmail open the message ready to go and the PDF is already copied (paste it with <kbd>Ctrl</kbd>+<kbd>V</kbd>); Outlook desktop attaches it itself.</dd>
            <dt>…extract a table that comes out badly?</dt>
            <dd>In the table window, switch engine: <strong>Model</strong> for tables in papers (borderless ones included), <strong>Ollama</strong> for scans. The rectangle has to cover the whole table.</dd>
            <dt>…fix a formula that was recognized badly?</dt>
            <dd>The LaTeX in the window is editable and the preview updates live. If it is several stacked equations, try “Multi-line”; otherwise switch engine.</dd>
            <dt>…take a passage from a paper into my notes, with the source?</dt>
            <dd>Select the text in the reader → “Send to Notes” (radial menu or the little bar): it arrives as a quotation with <code>[[@citekey]]</code> at the end, which becomes a backlink.</dd>
            <dt>…stop losing the notes I write about a group of papers?</dt>
            <dd>Link them to the collection: from the “Collection…” button above the note editor, or from the <strong>Notes</strong> section in the Archive panel, which also offers “+ Note with its N papers” to create one already linked and with the papers listed. In the list you can filter by collection — “No collection” shows the ones you never filed. Unlinking, or deleting the collection, never touches the <code>.md</code> file.</dd>
            <dt>…write a formula in a note?</dt>
            <dd>Write <code>$$ E = mc^2 $$</code>: you see it rendered in Side by side/Preview and in the exports.</dd>
            <dt>…put an image in a note?</dt>
            <dd>Drop it or paste it into the editor: it is saved as a file in <code>assets/</code> and only a short reference stays in the text.</dd>
            <dt>…export a note to PDF?</dt>
            <dd>Open the note → the <strong>PDF</strong> button → in the print window choose “Save as PDF”. MD, HTML and LaTeX are there too.</dd>
            <dt>…write a LaTeX article citing my library?</dt>
            <dd>Toolbar → Projects (LaTeX) → Create (pick a template). “Cite” inserts <code>\cite&#123;…&#125;</code> from the library, “Compile” produces the PDF with a preview. For an official template: download the .zip and use “From .zip…”.</dd>
            <dt>…compile if MiKTeX asks for Perl, or I have no compiler at all?</dt>
            <dd>With MiKTeX the compilation goes through texify and Perl is not needed. If you have nothing: <code>winget install Tectonic.Tectonic</code> (it fetches the packages itself).</dd>
            <dt>…find out which cited papers I am missing?</dt>
            <dd>Toolbar → Library care → Citation gaps. “Resolve reference DOIs” brings more of them into the count.</dd>
            <dt>…follow a topic and notice the new papers?</dt>
            <dd>Search online → ★ Save the search. At every start-up the new results collect in the New papers bell.</dd>
            <dt>…why is the Constellation empty, or missing the new papers?</dt>
            <dd>It needs the Semantic index: the toolbar icon builds and updates it. New documents (and notes) join at the next update.</dd>
            <dt>…reopen the last PDF where I left off?</dt>
            <dd>“Resume reading” on the toolbar (or radial menu/palette). Every PDF starts again from its last read page anyway.</dd>
            <dt>…why do the AI features not start?</dt>
            <dd>Look at the chip at the top: if it says “AI off”, click it. Then check under Settings → Local AI that the server (Ollama/LM Studio) is reachable and that the model exists.</dd>
            <dt>…free the GPU memory without shutting everything down?</dt>
            <dd>Toolbar → AI memory → “Free GPU”. “Stop AI”, on the other hand, shuts the server down as well and disables the AI.</dd>
            <dt>…find a command when I cannot remember where it lives?</dt>
            <dd><kbd>Ctrl</kbd>+<kbd>K</kbd> and type it: you get actions, documents, notes, wiki pages, projects and the sections of this guide — even while you are reading a PDF. It works in the radial menu too: open it and type to filter.</dd>
          </dl>
        </div>
        {/if}
