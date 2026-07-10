//! Export a note's Markdown to a self-contained HTML file or a LaTeX document.
//! Math is preserved (MathML in HTML via `wiki::render_html`, native LaTeX in the
//! `.tex`); embedded base64 figures are inlined (HTML) or extracted to sibling PNG
//! files (LaTeX). Pure string transforms — no AI, no network.

use base64::prelude::{Engine as _, BASE64_STANDARD};
use pulldown_cmark::{Alignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// A figure lifted out of a note's Markdown (a `data:` image URI), to be written
/// next to the exported `.tex` and referenced with `\includegraphics`.
pub struct ExtractedImage {
    pub filename: String,
    pub bytes: Vec<u8>,
}

/// The result of a LaTeX export: the `.tex` source plus the figure files it references.
pub struct LatexDoc {
    pub tex: String,
    pub images: Vec<ExtractedImage>,
}

/// Reduce `[[target]]` / `[[target|label]]` wikilinks to their visible label so an
/// export doesn't carry app-internal link syntax. Escaped `[\[…` (deliberately
/// neutralized quotes) are left alone — they render as literal brackets.
fn strip_wikilinks(md: &str) -> String {
    let re = regex::Regex::new(r"\[\[([^\]\[]+)\]\]").expect("valid regex");
    re.replace_all(md, |c: &regex::Captures| {
        let inner = &c[1];
        let label = inner.rsplit('|').next().unwrap_or(inner).trim();
        label.trim_start_matches('@').to_string()
    })
    .into_owned()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// A readable, self-contained stylesheet for the HTML export (no external assets).
const HTML_CSS: &str = r#"
:root { color-scheme: light dark; }
body { margin: 0; background: #fff; color: #1a1a1a; }
main { max-width: 46rem; margin: 0 auto; padding: 3rem 1.5rem 5rem;
  font: 16px/1.65 Georgia, "Times New Roman", serif; }
h1,h2,h3,h4 { font-family: system-ui, -apple-system, Segoe UI, sans-serif; line-height: 1.25; margin: 1.6em 0 .5em; }
h1 { font-size: 1.9em; } h2 { font-size: 1.5em; } h3 { font-size: 1.25em; }
p { margin: 0 0 1em; }
a { color: #2563eb; }
img { max-width: 100%; height: auto; border-radius: 6px; }
pre { background: #f4f4f5; padding: .8em 1em; border-radius: 8px; overflow-x: auto; }
code { font-family: ui-monospace, Consolas, monospace; font-size: .92em; }
pre code { font-size: .88em; }
blockquote { margin: 0 0 1em; padding: .2em 1em; border-left: 3px solid #d4d4d8; color: #52525b; }
table { border-collapse: collapse; margin: 1em 0; }
th, td { border: 1px solid #d4d4d8; padding: .4em .7em; text-align: left; }
math { font-size: 1.05em; }
math[display="block"] { display: block; margin: 1em 0; text-align: center; }
.mathraw { background: #fef2f2; color: #b91c1c; padding: 0 .3em; border-radius: 4px; }
hr { border: none; border-top: 1px solid #d4d4d8; margin: 2em 0; }
@media (prefers-color-scheme: dark) {
  body { background: #18181b; color: #e4e4e7; }
  a { color: #60a5fa; } pre { background: #27272a; }
  blockquote { border-left-color: #3f3f46; color: #a1a1aa; }
  th, td, hr { border-color: #3f3f46; }
}
"#;

/// Render a note to a standalone HTML document: the sanitized body (math as MathML,
/// figures inline as `data:` URIs) wrapped with an embedded stylesheet.
pub fn to_html(md: &str, title: &str) -> String {
    let body = crate::wiki::render_html(&strip_wikilinks(md));
    format!(
        "<!doctype html>\n<html lang=\"it\"><head><meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title>\n<style>{}</style>\n</head>\n<body><main>{}</main></body></html>\n",
        html_escape(title),
        HTML_CSS,
        body
    )
}

fn latex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '#' => out.push_str("\\#"),
            '$' => out.push_str("\\$"),
            '%' => out.push_str("\\%"),
            '&' => out.push_str("\\&"),
            '_' => out.push_str("\\_"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            _ => out.push(c),
        }
    }
    out
}

/// Escape a URL for use as a `\href`/`\includegraphics` argument so LaTeX-special
/// characters (notably `%`, which would comment out the rest of the line) can't break
/// compilation. Rare-in-URLs chars are made literal even if that changes their meaning
/// — the goal is a document that compiles.
fn latex_url_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '%' | '#' | '{' | '}' | '&' | '_' | '$' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Decode a `data:image/…;base64,…` URI into (extension, bytes). Returns None for
/// non-URIs and for formats pdflatex/graphicx can't include as a raster (e.g. SVG),
/// so those fall back to a placeholder rather than a mislabeled/broken figure.
fn decode_data_image(url: &str) -> Option<(&'static str, Vec<u8>)> {
    let rest = url.strip_prefix("data:image/")?;
    let (mime, after) = rest.split_once(";base64,")?;
    let ext = match mime.to_ascii_lowercase().as_str() {
        "png" => "png",
        "jpeg" | "jpg" => "jpg",
        "gif" => "gif",
        "webp" => "webp",
        _ => return None, // svg+xml, bmp, … — not embeddable as a raster figure
    };
    let bytes = BASE64_STANDARD.decode(after.trim()).ok()?;
    Some((ext, bytes))
}

const LATEX_PREAMBLE: &str = "\\documentclass[11pt]{article}\n\
\\usepackage[utf8]{inputenc}\n\
\\usepackage[T1]{fontenc}\n\
\\usepackage{amsmath,amssymb}\n\
\\usepackage{graphicx}\n\
\\usepackage{booktabs}\n\
\\usepackage[normalem]{ulem}\n\
\\usepackage[hidelinks]{hyperref}\n\
\\usepackage{parskip}\n\
\\begin{document}\n\n";

/// Convert a note's Markdown to a LaTeX document. `figures_dir` is the relative path
/// (from the `.tex`) where extracted figures live, e.g. `mynote_figures`.
pub fn to_latex(md: &str, figures_dir: &str) -> LatexDoc {
    let md = strip_wikilinks(md);
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_MATH);

    let mut out = String::from(LATEX_PREAMBLE);
    let mut images: Vec<ExtractedImage> = Vec::new();

    // Context flags.
    let mut in_code = false; // verbatim: emit text raw
    let mut list_ordered: Vec<bool> = Vec::new(); // nesting stack
    let mut image_alt: Option<String> = None; // Some => capturing an image's alt text
    let mut image_url = String::new();
    let mut cell_idx = 0usize; // column position within the current table row
    let mut in_table_head = false;

    for ev in Parser::new_ext(&md, opts) {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => {
                out.push_str(match level {
                    HeadingLevel::H1 => "\\section*{",
                    HeadingLevel::H2 => "\\subsection*{",
                    HeadingLevel::H3 => "\\subsubsection*{",
                    _ => "\\paragraph*{",
                });
            }
            Event::End(TagEnd::Heading(_)) => out.push_str("}\n\n"),
            Event::Start(Tag::Paragraph) => {}
            Event::End(TagEnd::Paragraph) => out.push_str("\n\n"),
            // Inline formatting is suppressed while capturing an image's alt text
            // (the alt is plain text) so no stray \textbf{}/\emph{} leaks before the figure.
            Event::Start(Tag::Emphasis) if image_alt.is_none() => out.push_str("\\emph{"),
            Event::End(TagEnd::Emphasis) if image_alt.is_none() => out.push('}'),
            Event::Start(Tag::Strong) if image_alt.is_none() => out.push_str("\\textbf{"),
            Event::End(TagEnd::Strong) if image_alt.is_none() => out.push('}'),
            Event::Start(Tag::Strikethrough) if image_alt.is_none() => out.push_str("\\sout{"),
            Event::End(TagEnd::Strikethrough) if image_alt.is_none() => out.push('}'),
            Event::Start(Tag::BlockQuote(_)) => out.push_str("\\begin{quote}\n"),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("\\end{quote}\n\n"),
            Event::Start(Tag::CodeBlock(_)) => {
                in_code = true;
                out.push_str("\\begin{verbatim}\n");
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                out.push_str("\\end{verbatim}\n\n");
            }
            Event::Start(Tag::List(first)) => {
                let ordered = first.is_some();
                list_ordered.push(ordered);
                out.push_str(if ordered { "\\begin{enumerate}\n" } else { "\\begin{itemize}\n" });
            }
            Event::End(TagEnd::List(_)) => {
                let ordered = list_ordered.pop().unwrap_or(false);
                out.push_str(if ordered { "\\end{enumerate}\n\n" } else { "\\end{itemize}\n\n" });
            }
            Event::Start(Tag::Item) => out.push_str("\\item "),
            Event::End(TagEnd::Item) => out.push('\n'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                out.push_str("\\href{");
                out.push_str(&latex_url_escape(&dest_url));
                out.push_str("}{");
            }
            Event::End(TagEnd::Link) => out.push('}'),
            Event::Start(Tag::Image { dest_url, .. }) => {
                image_alt = Some(String::new());
                image_url = dest_url.to_string();
            }
            Event::End(TagEnd::Image) => {
                let alt = image_alt.take().unwrap_or_default();
                if let Some((ext, bytes)) = decode_data_image(&image_url) {
                    let name = format!("fig{}.{ext}", images.len() + 1);
                    out.push_str("\\begin{figure}[h]\n\\centering\n");
                    out.push_str(&format!(
                        "\\includegraphics[width=0.8\\linewidth]{{{figures_dir}/{name}}}\n"
                    ));
                    if !alt.trim().is_empty() {
                        out.push_str(&format!("\\caption{{{}}}\n", latex_escape(alt.trim())));
                    }
                    out.push_str("\\end{figure}\n\n");
                    images.push(ExtractedImage { filename: name, bytes });
                } else if image_url.starts_with("data:") {
                    // Embedded image we can't include (e.g. SVG, or an undecodable blob):
                    // a placeholder, never the raw base64 dumped into the document.
                    let label = if alt.trim().is_empty() { "immagine non esportabile" } else { alt.trim() };
                    out.push_str(&format!("\\emph{{[{}]}}", latex_escape(label)));
                } else {
                    // Non-data image (external URL): keep a labelled link, not a broken graphic.
                    let label = if alt.trim().is_empty() { "immagine" } else { alt.trim() };
                    out.push_str(&format!("\\href{{{}}}{{{}}}", latex_url_escape(&image_url), latex_escape(label)));
                }
            }
            Event::Start(Tag::Table(aligns)) => {
                let spec: String = if aligns.is_empty() {
                    "l".into()
                } else {
                    aligns
                        .iter()
                        .map(|a| match a {
                            Alignment::Right => 'r',
                            Alignment::Center => 'c',
                            _ => 'l',
                        })
                        .collect()
                };
                out.push_str(&format!("\\begin{{center}}\n\\begin{{tabular}}{{{spec}}}\n\\toprule\n"));
                in_table_head = false;
            }
            Event::End(TagEnd::Table) => {
                out.push_str("\\bottomrule\n\\end{tabular}\n\\end{center}\n\n");
            }
            Event::Start(Tag::TableHead) => {
                in_table_head = true;
                cell_idx = 0;
            }
            Event::End(TagEnd::TableHead) => {
                out.push_str(" \\\\\n\\midrule\n");
                in_table_head = false;
            }
            Event::Start(Tag::TableRow) => cell_idx = 0,
            Event::End(TagEnd::TableRow) => {
                if !in_table_head {
                    out.push_str(" \\\\\n");
                }
            }
            Event::Start(Tag::TableCell) => {
                if cell_idx > 0 {
                    out.push_str(" & ");
                }
                cell_idx += 1;
            }
            Event::End(TagEnd::TableCell) => {}
            Event::Text(t) => {
                if let Some(alt) = image_alt.as_mut() {
                    alt.push_str(&t);
                } else if in_code {
                    out.push_str(&t);
                } else {
                    out.push_str(&latex_escape(&t));
                }
            }
            Event::Code(t) => {
                if let Some(alt) = image_alt.as_mut() {
                    alt.push_str(&t);
                } else {
                    out.push_str(&format!("\\texttt{{{}}}", latex_escape(&t)));
                }
            }
            Event::InlineMath(tex) => out.push_str(&format!("${}$", tex)),
            Event::DisplayMath(tex) => out.push_str(&format!("\\[{}\\]", tex)),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str("\\\\\n"),
            Event::Rule => out.push_str("\\par\\noindent\\hrulefill\\par\n\n"),
            Event::TaskListMarker(done) => {
                out.push_str(if done { "[$\\times$] " } else { "[ ] " });
            }
            // Raw HTML in a note is not carried into LaTeX.
            Event::Html(_) | Event::InlineHtml(_) | Event::FootnoteReference(_) => {}
            Event::Start(_) | Event::End(_) => {}
        }
    }

    out.push_str("\n\\end{document}\n");
    LatexDoc { tex: out, images }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 1×1 transparent PNG.
    const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

    fn sample() -> String {
        format!(
            "# Titolo\n\nTesto **grassetto** e *corsivo* con [[@einstein1905]].\n\n$$E = mc^2$$\n\n![Una figura]({PNG_1X1})\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n"
        )
    }

    #[test]
    fn html_export_is_self_contained_with_math_and_image() {
        let h = to_html(&sample(), "Titolo");
        assert!(h.starts_with("<!doctype html>"), "standalone doc");
        assert!(h.contains("<title>Titolo</title>"));
        assert!(h.contains("<math"), "math rendered to MathML");
        assert!(h.contains("data:image/png;base64,"), "figure inlined");
        assert!(h.contains("[[") == false, "wikilinks stripped: {}", &h[..h.len().min(0)]);
    }

    #[test]
    fn latex_export_escapes_urls_and_handles_edge_images() {
        // A percent in a URL must be escaped, or it comments out the rest of the .tex line.
        let d = to_latex("[link](http://e.com/a%20b) e ![a **b** c](http://e.com/x.png)", "f");
        assert!(d.tex.contains("a\\%20b"), "URL percent escaped: {}", d.tex);
        assert!(!d.tex.contains("\\textbf{}"), "no stray formatting leaked from image alt: {}", d.tex);
        // An SVG data image can't be a raster figure → placeholder, not a broken .png.
        let svg = to_latex("![diagramma](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)", "f");
        assert!(svg.images.is_empty(), "SVG not written as a raster file");
        assert!(svg.tex.contains("\\emph{[diagramma]}"), "SVG becomes a placeholder: {}", svg.tex);
    }

    #[test]
    fn latex_export_covers_structure_and_extracts_figures() {
        let d = to_latex(&sample(), "nota_figures");
        assert!(d.tex.contains("\\documentclass"), "preamble present");
        assert!(d.tex.contains("\\section*{Titolo}"));
        assert!(d.tex.contains("\\textbf{grassetto}"));
        assert!(d.tex.contains("\\emph{corsivo}"));
        assert!(d.tex.contains("\\[E = mc^2\\]"), "display math kept as LaTeX");
        assert!(d.tex.contains("\\includegraphics"), "figure referenced");
        assert!(d.tex.contains("nota_figures/fig1.png"));
        assert!(d.tex.contains("\\begin{tabular}"), "table -> tabular");
        assert!(d.tex.contains("einstein1905"), "wikilink reduced to its label");
        assert_eq!(d.images.len(), 1, "one figure extracted");
        assert_eq!(d.images[0].filename, "fig1.png");
        assert!(!d.images[0].bytes.is_empty(), "figure bytes decoded");
    }
}
