// Deterministic LaTeX generators for the reader's "→ LaTeX" actions: escape a
// text selection, or render an extracted table grid as a booktabs table. No AI,
// no network — pure string transforms, so the output is reliable. (Formula → LaTeX
// is a separate, model-based feature.)

// The characters LaTeX treats specially, each mapped to its literal-text form.
// Backslash and caret/tilde need a command; the rest just take a leading backslash.
const LATEX_ESCAPES: Record<string, string> = {
  "\\": "\\textbackslash{}",
  "{": "\\{",
  "}": "\\}",
  "&": "\\&",
  "%": "\\%",
  "$": "\\$",
  "#": "\\#",
  "_": "\\_",
  "~": "\\textasciitilde{}",
  "^": "\\textasciicircum{}",
};

/** Escape a plain-text string so it typesets verbatim in LaTeX. Single-pass: each
 *  replacement is emitted literally and never re-scanned, so it can't double-escape. */
export function escapeLatex(s: string): string {
  return (s ?? "").replace(/[\\{}&%$#_~^]/g, (c) => LATEX_ESCAPES[c]);
}

/** A selected paragraph as escaped LaTeX, with runs of blank lines collapsed to the
 *  paragraph break LaTeX expects (one empty line). */
export function textToLatex(s: string): string {
  const body = (s ?? "").replace(/\r\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
  return escapeLatex(body);
}

// ----- The rich-Markdown subset emitted by the region-text extractor -----
// The backend renders extracted paper text as Markdown carrying only: `*italic*`,
// `**bold**`, `***both***`, `<sup>…</sup>`, `<sub>…</sub>`, backslash escapes
// (\* \_ \` \[ \\ and a guarded leading \# \> \- \+ \.) and the entities
// &lt;/&amp;. The two converters below walk exactly that subset, so a formatted
// extraction exports cleanly as plain text or LaTeX too.

type RichMark = "***" | "**" | "*" | "<sup>" | "</sup>" | "<sub>" | "</sub>";
const RICH_MARKS: readonly RichMark[] = ["<sup>", "</sup>", "<sub>", "</sub>", "***", "**", "*"];

/** Tokenize the extractor's rich-Markdown into text pieces and style markers. */
function walkRichMd(md: string, emit: { text: (t: string) => void; mark: (m: RichMark) => void }): void {
  const s = md ?? "";
  let i = 0;
  let buf = "";
  const flush = () => {
    if (buf) emit.text(buf);
    buf = "";
  };
  while (i < s.length) {
    const c = s[i];
    if (c === "\\" && i + 1 < s.length) {
      buf += s[i + 1]; // backslash escape → the literal character
      i += 2;
      continue;
    }
    if (s.startsWith("&lt;", i)) { buf += "<"; i += 4; continue; }
    if (s.startsWith("&amp;", i)) { buf += "&"; i += 5; continue; }
    const mark = RICH_MARKS.find((m) => s.startsWith(m, i));
    if (mark) {
      flush();
      emit.mark(mark);
      i += mark.length;
      continue;
    }
    buf += c;
    i += 1;
  }
  flush();
}

/** Strip the extractor's rich-Markdown down to plain text (markers removed,
 *  escapes and entities resolved). */
export function richMdToPlain(md: string): string {
  let out = "";
  walkRichMd(md, { text: (t) => (out += t), mark: () => {} });
  return out;
}

/** Convert the extractor's rich-Markdown to LaTeX: italic/bold become
 *  \textit/\textbf, sup/sub become \textsuperscript/\textsubscript, and the
 *  text itself is LaTeX-escaped. */
export function richMdToLatex(md: string): string {
  let out = "";
  const open: string[] = [];
  const toggle = (cmd: string) => {
    if (open[open.length - 1] === cmd) {
      out += "}";
      open.pop();
    } else {
      out += `\\${cmd}{`;
      open.push(cmd);
    }
  };
  walkRichMd(md.replace(/\r\n/g, "\n").replace(/\n{3,}/g, "\n\n").trim(), {
    text: (t) => (out += escapeLatex(t)),
    mark: (m) => {
      if (m === "*") toggle("textit");
      else if (m === "**") toggle("textbf");
      else if (m === "***") {
        // Open/close both; closing order mirrors opening.
        if (open[open.length - 1] === "textit" && open[open.length - 2] === "textbf") {
          out += "}}";
          open.pop();
          open.pop();
        } else {
          out += "\\textbf{\\textit{";
          open.push("textbf", "textit");
        }
      } else if (m === "<sup>") { out += "\\textsuperscript{"; open.push("sup"); }
      else if (m === "<sub>") { out += "\\textsubscript{"; open.push("sub"); }
      else if (m === "</sup>" || m === "</sub>") {
        // Close only a matching opener: a stray closer (possible after a hand
        // edit of the MD) must not emit an unbalanced brace.
        if (open[open.length - 1] === (m === "</sup>" ? "sup" : "sub")) {
          out += "}";
          open.pop();
        }
      }
    },
  });
  while (open.length) {
    out += "}";
    open.pop();
  }
  return out;
}

/** Render a table grid (rows of cells) as a `booktabs` table float. The first row
 *  is treated as the header when there is more than one row. Requires
 *  \usepackage{booktabs} in the document. */
export function tableToLatex(grid: string[][]): string {
  const rows = (grid ?? []).filter((r) => r && r.length);
  if (!rows.length) return "";
  const ncol = Math.max(...rows.map((r) => r.length));
  const fmt = (r: string[]) => {
    const cells = r.map((c) => escapeLatex((c ?? "").trim()));
    while (cells.length < ncol) cells.push("");
    return cells.join(" & ") + " \\\\";
  };
  const body: string[] = ["\\toprule"];
  if (rows.length > 1) {
    body.push(fmt(rows[0]), "\\midrule", ...rows.slice(1).map(fmt));
  } else {
    body.push(...rows.map(fmt));
  }
  body.push("\\bottomrule");
  const ind = (lines: string[], n: number) => lines.map((l) => " ".repeat(n) + l);
  const tabular = [
    "\\begin{tabular}{@{}" + "l".repeat(ncol) + "@{}}",
    ...ind(body, 2),
    "\\end{tabular}",
  ];
  return [
    "\\begin{table}[htbp]",
    ...ind(["\\centering", "\\caption{}", "\\label{tab:}", ...tabular], 2),
    "\\end{table}",
  ].join("\n");
}

/** Render a table grid as a GitHub-flavored Markdown table. The first row is the
 *  header (with a `---` separator) when there is more than one row. Pipes and
 *  newlines inside cells are escaped so they don't break the table. */
export function tableToMarkdown(grid: string[][]): string {
  const rows = (grid ?? []).filter((r) => r && r.length);
  if (!rows.length) return "";
  const ncol = Math.max(...rows.map((r) => r.length));
  const esc = (c: string) => (c ?? "").replace(/\r?\n/g, " ").replace(/\|/g, "\\|").trim();
  const fmt = (r: string[]) => {
    const cells = r.map(esc);
    while (cells.length < ncol) cells.push("");
    return "| " + cells.join(" | ") + " |";
  };
  const sep = "| " + Array(ncol).fill("---").join(" | ") + " |";
  const out = [fmt(rows[0]), sep];
  for (const r of rows.slice(1)) out.push(fmt(r));
  return out.join("\n");
}

/** Render a table grid as CSV (RFC 4180-ish): fields with a comma, quote, or
 *  newline are wrapped in double quotes and internal quotes are doubled. */
export function tableToCsv(grid: string[][]): string {
  const rows = (grid ?? []).filter((r) => r && r.length);
  if (!rows.length) return "";
  const ncol = Math.max(...rows.map((r) => r.length));
  const cell = (c: string) => {
    const v = c ?? "";
    return /[",\n\r]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
  };
  return rows
    .map((r) => {
      const cells = r.map(cell);
      while (cells.length < ncol) cells.push(""); // pad ragged rows to equal field count
      return cells.join(",");
    })
    .join("\n");
}
