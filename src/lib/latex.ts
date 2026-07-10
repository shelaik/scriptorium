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
