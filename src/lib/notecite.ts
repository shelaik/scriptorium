// Build the Markdown block inserted into a note when the user sends a PDF
// selection, an abstract, or an AI summary "to a note". The block is a blockquote
// with the attribution on its last line (the "tutto in un blocco" style the user
// picked). The attribution is a live wikilink to the source paper — `[[@citekey]]`
// when a citekey exists (else `[[Title]]`), both of which the note renderer
// (notes::weave_note_links) resolves to the paper.

export interface NotePayload {
  /** The text to quote (selection, abstract, or summary). */
  content: string;
  /** Source paper's citekey, when known — yields the tightest backlink. */
  citekey?: string | null;
  /** Source paper's title — the backlink fallback and the "new note" name. */
  title: string;
  /** 1-based PDF page, for a selection; omitted for abstract/summary. */
  page?: number | null;
  /** Attribution label, e.g. "Abstract di" / "Riassunto AI di". Omit for a plain quote. */
  label?: string | null;
  /** Collapse internal whitespace (good for PDF selections, whose line breaks are
   *  layout noise). Leave false to preserve structure (AI summaries). */
  collapse?: boolean;
  /** When set (e.g. "latex"), render the content as a fenced code block in that
   *  language instead of a blockquote, with the attribution as a caption below. */
  code?: string | null;
}

/** A wikilink token that resolves back to the source paper. */
export function refToken(citekey: string | null | undefined, title: string): string {
  const ck = citekey?.trim();
  if (ck) return `[[@${ck}]]`;
  // Titles are matched case-insensitively; strip the few chars that break `[[…]]`.
  const safe = (title || "").replace(/[[\]|]/g, "").trim();
  return `[[${safe || "senza titolo"}]]`;
}

/** A fenced code block (e.g. LaTeX) with the source paper as a caption line below.
 *  The content stays verbatim inside the fence; the `[[@citekey]]` caption sits
 *  outside so it resolves to a live backlink. */
function buildCodeBlock(p: NotePayload): string {
  const body = (p.content ?? "").replace(/\r\n/g, "\n").replace(/\n+$/, "");
  // The fence must be longer than the longest backtick run inside, or the block
  // would close early (LaTeX never has these, but extracted text is untrusted).
  const longest = (body.match(/`+/g) ?? []).reduce((m, r) => Math.max(m, r.length), 0);
  const fence = "`".repeat(Math.max(3, longest + 1));
  const block = `${fence}${p.code}\n${body}\n${fence}`;
  const ref = refToken(p.citekey, p.title);
  const label = p.label && p.label.trim() ? `${p.label.trim()} ` : "";
  const page = p.page != null ? `, p. ${p.page}` : "";
  return `${block}\n\n*${label}${ref}${page}*`;
}

/** The Markdown block to append to a note. */
export function buildQuoteBlock(p: NotePayload): string {
  if (p.code) return buildCodeBlock(p);
  let text = (p.content ?? "").replace(/\r\n/g, "\n");
  if (p.collapse) {
    text = text.replace(/\s+/g, " ").trim();
  } else {
    text = text
      .split("\n")
      .map((l) => l.replace(/[ \t]+$/g, ""))
      .join("\n")
      .trim();
  }
  // Neutralize wikilink syntax in the QUOTED body: a selection/abstract/summary
  // that happens to contain `[[…]]` must not become a live link or pollute the
  // backlink graph. Backslash-escaping breaks the `[[`/`]]` the note renderer
  // looks for, while still rendering as literal brackets. The attribution built
  // below keeps its deliberate, live `[[@citekey]]`.
  text = text.replace(/\[\[/g, "[\\[").replace(/\]\]/g, "]\\]");
  const body = text
    .split("\n")
    .map((l) => (l.trim() ? `> ${l}` : ">"))
    .join("\n");

  const ref = refToken(p.citekey, p.title);
  let attribution: string;
  if (p.label && p.label.trim()) {
    attribution = `> — ${p.label.trim()} ${ref}`;
  } else if (p.page != null) {
    attribution = `> — ${ref}, p. ${p.page}`;
  } else {
    attribution = `> — ${ref}`;
  }
  return `${body}\n>\n${attribution}`;
}
