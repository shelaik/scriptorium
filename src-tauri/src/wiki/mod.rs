//! "Wiki della libreria": concept pages synthesized by the local LLM from the
//! RAG passage index, with numbered sources that deep-link into the PDFs and
//! [[links]] between pages.
//!
//! This module holds the pure, testable pieces — slugs, prompts, citation
//! post-processing, source-coverage checking, link weaving, markdown
//! rendering. The orchestration (retrieval, LLM calls, DB writes, progress
//! events) lives in `commands.rs` next to the other AI commands.

use std::collections::HashSet;

/// URL/id-safe slug for a concept ("Large Language Models" → "large-language-models").
pub fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = true; // trim leading dashes
    for c in s.chars() {
        let c = c.to_lowercase().next().unwrap_or(c);
        if c.is_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "pagina".into()
    } else {
        out
    }
}

/// Step 1 — per-paper claim extraction. A narrow, grounded task: small local
/// models are far more reliable here than at free-form writing.
pub fn extraction_prompt(concept: &str, title: &str, year: Option<i64>, material: &str) -> String {
    let year = year.map(|y| format!(" ({y})")).unwrap_or_default();
    format!(
        "Sei un assistente di ricerca. Dal materiale seguente, tratto dal paper «{title}»{year}, \
         estrai da 3 a 6 affermazioni chiave pertinenti al concetto «{concept}».\n\
         Regole:\n\
         - ogni affermazione in italiano, autonoma e fattuale, su UNA riga che inizia con \"- \";\n\
         - se il passaggio da cui deriva indica la pagina (es. [p. 4]), chiudi la riga con (p. 4);\n\
         - usa SOLO il materiale fornito: niente conoscenze esterne, niente opinioni;\n\
         - se il materiale non dice nulla di pertinente al concetto, rispondi esattamente: NIENTE.\n\n\
         MATERIALE:\n{material}"
    )
}

/// Step 2 — page synthesis, exclusively from the extracted claims.
pub fn synthesis_prompt(concept: &str, claims_blocks: &[String]) -> String {
    format!(
        "Scrivi in italiano la pagina di un'enciclopedia personale sul concetto «{concept}», \
         basandoti ESCLUSIVAMENTE sulle affermazioni estratte dai paper elencati sotto.\n\
         Struttura e regole:\n\
         - NON inserire un titolo iniziale: comincia direttamente con \"## In breve\" (2-3 frasi);\n\
         - poi \"## Nei documenti\": il cuore della pagina, cosa dice ciascun filone, con confronti;\n\
         - poi, solo se emergono, \"## Punti aperti o in tensione\";\n\
         - OGNI fonte elencata deve comparire almeno una volta: cita con [1], [2]… subito dopo \
           ogni affermazione che ne deriva (obbligatorio);\n\
         - quando un'affermazione riporta la pagina, mantienila nel testo come (p. N);\n\
         - tono enciclopedico ma vivo, 350-550 parole, niente premesse né conclusioni di cortesia;\n\
         - non inventare nulla che non sia nelle affermazioni.\n\n\
         AFFERMAZIONI PER PAPER:\n\n{blocks}",
        blocks = claims_blocks.join("\n\n")
    )
}

/// Step 3 — coverage repair: the synthesis ignored some sources.
pub fn repair_prompt(concept: &str, page_md: &str, missing: &[(usize, String)]) -> String {
    let list = missing
        .iter()
        .map(|(n, t)| format!("[{n}] {t}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "La pagina qui sotto sul concetto «{concept}» NON cita queste fonti:\n{list}\n\n\
         Integra ciascuna fonte mancante con 1-2 frasi dove è pertinente (mantenendo le citazioni \
         [n] esistenti), oppure — se davvero non è pertinente — aggiungi in fondo una sezione \
         \"## Fonti non pertinenti\" elencandola come \"[n] — motivo in una riga\". \
         Restituisci la pagina COMPLETA aggiornata, senza commenti aggiuntivi.\n\n\
         PAGINA:\n{page_md}"
    )
}

/// Parse the extraction output into claims; `(p. N)` at end of line becomes the page.
pub fn parse_claims(text: &str) -> Vec<(String, Option<i64>)> {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(claim) = line.strip_prefix("- ").or_else(|| line.strip_prefix("• ")) else {
            continue;
        };
        let claim = claim.trim();
        if claim.is_empty() {
            continue;
        }
        let (text, page) = match claim.rfind("(p.") {
            Some(i) if claim.ends_with(')') => {
                let n: String = claim[i + 3..claim.len() - 1]
                    .chars()
                    .filter(|c| c.is_ascii_digit())
                    .collect();
                (claim[..i].trim_end().to_string(), n.parse::<i64>().ok())
            }
            _ => (claim.to_string(), None),
        };
        out.push((text, page));
    }
    out
}

/// The source numbers `[n]` actually cited in the page.
pub fn cited_ns(md: &str) -> HashSet<usize> {
    let mut out = HashSet::new();
    let bytes = md.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let mut j = i + 1;
            let mut num = 0usize;
            let mut any = false;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                num = num * 10 + (bytes[j] - b'0') as usize;
                any = true;
                j += 1;
            }
            if any && j < bytes.len() && bytes[j] == b']' {
                out.insert(num);
                i = j;
            }
        }
        i += 1;
    }
    out
}

/// Turn bare `[n]` citations into markdown links `[\[n\]](#src-n)` the UI can
/// intercept (open source n at its page). Skips `[n]` that are already link
/// text or immediately followed by a link target.
pub fn link_citations(md: &str) -> String {
    let mut out = String::with_capacity(md.len() + 64);
    let bytes = md.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            let mut j = i + 1;
            while j < bytes.len() && bytes[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < bytes.len() && bytes[j] == b']' {
                let already_link = bytes.get(j + 1) == Some(&b'(');
                let escaped = i > 0 && bytes[i - 1] == b'\\';
                if !already_link && !escaped {
                    let n = &md[i + 1..j];
                    out.push_str(&format!("[\\[{n}\\]](#src-{n})"));
                    i = j + 1;
                    continue;
                }
            }
        }
        // advance one full UTF-8 char
        let ch_len = utf8_len(bytes[i]);
        out.push_str(&md[i..i + ch_len]);
        i += ch_len;
    }
    out
}

fn utf8_len(b: u8) -> usize {
    match b {
        b if b >= 0xF0 => 4,
        b if b >= 0xE0 => 3,
        b if b >= 0xC0 => 2,
        _ => 1,
    }
}

/// Weave [[links]] to the OTHER wiki pages: the first case-insensitive,
/// word-boundary occurrence of each other page's concept becomes a
/// `[testo](#wiki-slug)` link. Conservative: plain-text lines only (headings,
/// existing links and code spans are left alone), names shorter than 4 chars skipped.
pub fn weave_links(md: &str, others: &[(String, String)]) -> String {
    let mut done: HashSet<&str> = HashSet::new();
    let mut lines: Vec<String> = md.lines().map(str::to_string).collect();
    for (name, slug) in others {
        if name.chars().count() < 4 {
            continue;
        }
        'lines: for line in lines.iter_mut() {
            let t = line.trim_start();
            if t.starts_with('#') || t.starts_with("```") || line.contains('[') {
                continue; // keep it simple: never touch headings or lines with links
            }
            // ASCII-lowercase is byte-length-preserving, so offsets into `lower` stay
            // valid for slicing the original `line`. (Unicode to_lowercase() is NOT
            // length-preserving — e.g. 'İ' → 2 bytes — and would panic on a slice.)
            let lower = line.to_ascii_lowercase();
            let needle = name.to_ascii_lowercase();
            let mut from = 0;
            while let Some(pos) = lower[from..].find(&needle) {
                let start = from + pos;
                let end = start + needle.len();
                let ok_before = start == 0
                    || !lower[..start].chars().next_back().unwrap_or(' ').is_alphanumeric();
                let ok_after =
                    end >= lower.len() || !lower[end..].chars().next().unwrap_or(' ').is_alphanumeric();
                if ok_before && ok_after {
                    if done.contains(slug.as_str()) {
                        break 'lines;
                    }
                    let orig = &line[start..end];
                    let replaced = format!("[{orig}](#wiki-{slug})");
                    line.replace_range(start..end, &replaced);
                    done.insert(slug.as_str());
                    break 'lines;
                }
                from = end;
            }
        }
    }
    lines.join("\n")
}

/// MathML tags emitted by latex2mathml — allowed through ammonia so rendered math
/// survives sanitization and the webview draws it natively.
const MATHML_TAGS: [&str; 30] = [
    "math", "semantics", "annotation", "mrow", "mi", "mo", "mn", "ms", "mtext", "mspace", "msup",
    "msub", "msubsup", "mfrac", "msqrt", "mroot", "mover", "munder", "munderover", "mmultiscripts",
    "mtable", "mtr", "mtd", "mstyle", "mpadded", "mphantom", "menclose", "mfenced", "mprescripts",
    "none",
];
/// Presentational MathML attributes — inert on HTML tags, so allowing them globally
/// carries no XSS surface (no `href`/`src`/`on*`).
const MATHML_ATTRS: [&str; 27] = [
    "mathvariant", "displaystyle", "scriptlevel", "display", "xmlns", "encoding", "stretchy",
    "fence", "separator", "accent", "accentunder", "form", "lspace", "rspace", "linethickness",
    "columnalign", "rowalign", "columnspacing", "rowspacing", "open", "close", "notation", "width",
    "mathsize", "dir", "largeop", "movablelimits",
];

/// Convert one LaTeX math fragment to MathML. Multi-line environments latex2mathml
/// can't parse (gathered/aligned/…) are split into stacked block equations. On any
/// failure the raw LaTeX is kept (as a styled code span) so a formula is never lost.
fn render_math(latex: &str, block: bool) -> String {
    let t = latex.trim();
    const ENVS: [&str; 8] = [
        "gathered", "aligned", "align*", "align", "gather*", "gather", "split", "eqnarray",
    ];
    for env in ENVS {
        let begin = format!("\\begin{{{env}}}");
        let end = format!("\\end{{{env}}}");
        if let (Some(bi), Some(ei)) = (t.find(&begin), t.rfind(&end)) {
            if ei >= bi + begin.len() {
                let inner = &t[bi + begin.len()..ei];
                let mut out = String::new();
                for line in inner.split("\\\\") {
                    let line = line.replace('&', " ");
                    let line = line.trim();
                    if !line.is_empty() {
                        out.push_str(&render_one(line, true));
                    }
                }
                if !out.is_empty() {
                    return out;
                }
            }
        }
    }
    render_one(t, block)
}

fn render_one(latex: &str, block: bool) -> String {
    let style = if block {
        latex2mathml::DisplayStyle::Block
    } else {
        latex2mathml::DisplayStyle::Inline
    };
    match latex2mathml::latex_to_mathml(latex, style) {
        Ok(m) => m,
        Err(_) => {
            let (o, c) = if block { ("$$", "$$") } else { ("$", "$") };
            let esc = latex.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
            format!("<code class=\"mathraw\">{o}{esc}{c}</code>")
        }
    }
}

/// Markdown → sanitized HTML (same recipe as the GitHub README preview: pulldown-cmark
/// then ammonia, which keeps fragment hrefs like `#src-1`). `$…$` / `$$…$$` math is
/// rendered to MathML; embedded `data:image/…` figures are kept on `<img src>`.
pub fn render_html(md: &str) -> String {
    use pulldown_cmark::{html, Event, Options, Parser};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_MATH);
    let parser = Parser::new_ext(md, opts).map(|ev| match ev {
        Event::InlineMath(tex) => Event::InlineHtml(render_math(&tex, false).into()),
        Event::DisplayMath(tex) => Event::Html(render_math(&tex, true).into()),
        other => other,
    });
    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, parser);
    // Allow inline `data:` image URIs so embedded figures (Figura → Appunti) render,
    // but ONLY on `<img src>` — every other URL attribute (e.g. `<a href>`) keeps the
    // default safe schemes, so a `data:text/html` link can't slip through. MathML tags
    // (from render_math) are also allowed so rendered formulas survive sanitization.
    let mut b = ammonia::Builder::default();
    b.add_url_schemes(["data"]);
    b.add_tags(MATHML_TAGS);
    b.add_generic_attributes(MATHML_ATTRS);
    b.attribute_filter(|element, attribute, value| {
        // Normalize like the URL parser (trim leading whitespace, lowercase the
        // scheme) so a mixed-case `DATA:` or ` data:` can't slip past on a non-img
        // attribute after `data` was added to the global scheme allowlist.
        let v = value.trim_start().to_ascii_lowercase();
        if v.starts_with("data:")
            && !(element == "img" && attribute == "src" && v.starts_with("data:image/"))
        {
            return None;
        }
        Some(value.into())
    });
    b.clean(&unsafe_html).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weave_links_no_panic_on_length_changing_char() {
        // 'İ' (U+0130) lowercases to 2 chars under Unicode rules; a byte-offset from a
        // Unicode-lowercased copy would panic when slicing the original line. Must not.
        let others = vec![("learning".to_string(), "learning".to_string())];
        let out = weave_links("İstanbul e il learning profondo", &others);
        assert!(out.contains("[learning](#wiki-learning)"), "should link without panic: {out}");
    }

    #[test]
    fn math_in_render_html() {
        // Inline and display math become MathML.
        let h = render_html("Energia $E=mc^2$ e blocco:\n\n$$\\frac{a}{b}$$\n");
        assert!(h.contains("<math"), "inline/display math should emit MathML: {h}");
        assert!(h.matches("<math").count() >= 2, "both inline and block math: {h}");
        // A gathered block (multi-formula OCR output) stacks into several equations.
        let g = render_html("$$\\begin{gathered} a = b \\\\ c = d \\end{gathered}$$");
        assert!(g.matches("<math").count() >= 2, "gathered should stack: {g}");
        // A data: image survives, code fences are NOT math-rendered.
        let c = render_html("```\n$x=1$\n```\n");
        assert!(!c.contains("<math"), "math inside a code fence must stay literal: {c}");
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("Large Language Models"), "large-language-models");
        assert_eq!(slugify("  Dissonanza cognitiva! "), "dissonanza-cognitiva");
        assert_eq!(slugify("???"), "pagina");
    }

    #[test]
    fn claims_parse_with_pages() {
        let text = "- Prima affermazione (p. 4)\n- Seconda senza pagina\nrumore\n- Terza (p. 12)";
        let c = parse_claims(text);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0], ("Prima affermazione".into(), Some(4)));
        assert_eq!(c[1], ("Seconda senza pagina".into(), None));
        assert_eq!(c[2], ("Terza".into(), Some(12)));
    }

    #[test]
    fn citations_found_and_linked() {
        let md = "Frase uno [1]. Frase due [2, 3]? No: [12].";
        let ns = cited_ns(md);
        assert!(ns.contains(&1) && ns.contains(&12) && !ns.contains(&2)); // [2, 3] non è [n] puro
        let linked = link_citations(md);
        assert!(linked.contains("[\\[1\\]](#src-1)"));
        assert!(linked.contains("[\\[12\\]](#src-12)"));
        // idempotenza sui link già creati: il testo "\[1\]" è escaped e non viene ri-avvolto
        assert_eq!(link_citations(&linked), linked);
    }

    #[test]
    fn weaving_links_other_pages() {
        let md = "## In breve\nIl reinforcement learning guida gli agenti.\nAncora reinforcement learning qui.";
        let out = weave_links(md, &[("reinforcement learning".into(), "reinforcement-learning".into())]);
        assert_eq!(out.matches("(#wiki-reinforcement-learning)").count(), 1); // solo la prima occorrenza
        assert!(out.contains("[reinforcement learning](#wiki-reinforcement-learning)"));
        assert!(out.starts_with("## In breve")); // l'heading non viene toccato
    }

    #[test]
    fn html_keeps_fragment_links() {
        let html = render_html("Testo [\\[1\\]](#src-1) e [altro](#wiki-slug).");
        assert!(html.contains("href=\"#src-1\""));
        assert!(html.contains("href=\"#wiki-slug\""));
        assert!(!html.contains("<script"));
    }
}
