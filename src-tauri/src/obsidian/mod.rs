//! Build a Markdown note (Obsidian / Logseq / Zettlr-compatible) for a document.
//! Pure string construction — the DB gathering lives in the command layer.

/// One highlight/annotation rendered into the note.
pub struct NoteAnnotation {
    pub page: i64,
    pub quote: Option<String>,
    pub note: Option<String>,
}

/// Everything needed to render one document as a Markdown note.
pub struct NoteData {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    pub tags: Vec<String>,
    pub added: Option<String>,
    pub favorite: bool,
    pub pdf_path: Option<String>,
    pub abstract_text: Option<String>,
    pub summary: Option<String>,
    pub notes: Option<String>,
    pub annotations: Vec<NoteAnnotation>,
}

/// A filesystem-safe note filename stem (no extension), never empty.
pub fn safe_filename(title: &str, fallback_id: i64) -> String {
    // Strip characters illegal on Windows/macOS and Obsidian-reserved ones.
    let cleaned: String = title
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']' => ' ',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let trimmed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = trimmed.trim_end_matches('.').trim();
    if trimmed.is_empty() {
        return format!("documento-{fallback_id}");
    }
    // Keep filenames reasonable on all filesystems.
    let stem: String = trimmed.chars().take(120).collect();
    let stem = stem.trim().to_string();
    // Avoid Windows reserved device names (CON, NUL, COM1…), which resolve to the
    // device even with a `.md` extension and would make the write fail.
    const RESERVED: &[&str] = &[
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let base = stem.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&base.as_str()) {
        format!("_{stem}")
    } else {
        stem
    }
}

/// Replace control chars and Unicode line/paragraph separators with spaces so a
/// value can't break out of a quoted YAML scalar onto a new line.
fn yaml_sanitize(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() || matches!(c, '\u{0085}' | '\u{2028}' | '\u{2029}') { ' ' } else { c })
        .collect()
}

/// Escape a string as a double-quoted YAML scalar.
fn yaml_str(s: &str) -> String {
    let clean = yaml_sanitize(s);
    format!("\"{}\"", clean.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A `[[wikilink]]` as a quoted YAML scalar (Obsidian renders links in properties).
fn yaml_wikilink(name: &str) -> String {
    // Inner brackets would break the wikilink; soften them to parens.
    let safe = yaml_sanitize(name).replace('[', "(").replace(']', ")");
    format!("\"[[{}]]\"", safe.trim())
}

/// Render the full Markdown note for one document.
pub fn build_markdown(d: &NoteData) -> String {
    let mut s = String::new();

    // ----- YAML frontmatter -----
    s.push_str("---\n");
    s.push_str(&format!("title: {}\n", yaml_str(&d.title)));
    if !d.authors.is_empty() {
        s.push_str("authors:\n");
        for a in &d.authors {
            s.push_str(&format!("  - {}\n", yaml_wikilink(a)));
        }
    }
    if let Some(y) = d.year {
        s.push_str(&format!("year: {y}\n"));
    }
    if let Some(v) = &d.venue {
        if !v.trim().is_empty() {
            s.push_str(&format!("venue: {}\n", yaml_str(v)));
        }
    }
    if let Some(doi) = &d.doi {
        if !doi.trim().is_empty() {
            s.push_str(&format!("doi: {}\n", yaml_str(doi)));
        }
    }
    if !d.tags.is_empty() {
        s.push_str("tags:\n");
        for t in &d.tags {
            s.push_str(&format!("  - {}\n", yaml_wikilink(t)));
        }
    }
    if let Some(a) = &d.added {
        if !a.trim().is_empty() {
            s.push_str(&format!("added: {}\n", yaml_str(a)));
        }
    }
    s.push_str(&format!("favorite: {}\n", d.favorite));
    if let Some(p) = &d.pdf_path {
        if !p.trim().is_empty() {
            s.push_str(&format!("pdf: {}\n", yaml_str(p)));
        }
    }
    s.push_str("source: Scriptorium\n");
    s.push_str("---\n\n");

    // ----- Body -----
    s.push_str(&format!("# {}\n\n", d.title));

    if let Some(p) = &d.pdf_path {
        if !p.trim().is_empty() {
            // file:// link so the PDF opens from the note.
            // Angle-bracketed destination (CommonMark) tolerates spaces in the path.
            let url = format!("file:///{}", p.replace('\\', "/"));
            s.push_str(&format!("[📄 Apri il PDF](<{}>)\n\n", url));
        }
    }

    if let Some(ab) = &d.abstract_text {
        if !ab.trim().is_empty() {
            s.push_str("## Abstract\n\n");
            s.push_str(ab.trim());
            s.push_str("\n\n");
        }
    }
    if let Some(sum) = &d.summary {
        if !sum.trim().is_empty() {
            s.push_str("## Riassunto\n\n");
            s.push_str(sum.trim());
            s.push_str("\n\n");
        }
    }
    if let Some(n) = &d.notes {
        if !n.trim().is_empty() {
            s.push_str("## Note\n\n");
            s.push_str(n.trim());
            s.push_str("\n\n");
        }
    }
    if !d.annotations.is_empty() {
        s.push_str("## Annotazioni\n\n");
        for a in &d.annotations {
            let quote = a.quote.as_deref().map(str::trim).filter(|q| !q.is_empty());
            let note = a.note.as_deref().map(str::trim).filter(|n| !n.is_empty());
            match (quote, note) {
                (Some(q), Some(n)) => s.push_str(&format!("- **p.{}** «{}» — {}\n", a.page, q, n)),
                (Some(q), None) => s.push_str(&format!("- **p.{}** «{}»\n", a.page, q)),
                (None, Some(n)) => s.push_str(&format!("- **p.{}** {}\n", a.page, n)),
                (None, None) => {}
            }
        }
        s.push('\n');
    }

    s
}
