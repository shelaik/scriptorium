//! Citation export: BibTeX / BibLaTeX / RIS / CSL-JSON, plus a couple of
//! formatted human styles (APA, IEEE). All generated locally, no network.

use serde_json::json;

/// Minimal bibliographic record for citation output.
pub struct CiteItem {
    pub title: Option<String>,
    /// (given, family) pairs in author order.
    pub authors: Vec<(Option<String>, Option<String>)>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    /// The persisted, library-unique citation key. When present it is emitted
    /// verbatim (BibTeX entry key, CSL id, \cite{}, @pandoc); when absent we
    /// fall back to the computed [`citekey`]. See `db::citekey`.
    pub citekey: Option<String>,
}

fn alnum_lower(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(|c| c.to_lowercase()).collect()
}

/// Stable-ish BibTeX citekey from raw parts: firstauthorfamily + year + first
/// title word (>3 chars). Shared with the DB layer that persists the key.
pub fn citekey_from_parts(family: Option<&str>, year: Option<i64>, title: Option<&str>) -> String {
    let fam = family
        .map(alnum_lower)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "anon".to_string());
    let year = year.map(|y| y.to_string()).unwrap_or_default();
    let word = title
        .unwrap_or("")
        .split_whitespace()
        .map(alnum_lower)
        .find(|w| w.len() > 3)
        .unwrap_or_default();
    format!("{fam}{year}{word}")
}

/// Computed citekey for an item (ignores any stored key).
pub fn citekey(item: &CiteItem) -> String {
    citekey_from_parts(
        item.authors.first().and_then(|(_, f)| f.as_deref()),
        item.year,
        item.title.as_deref(),
    )
}

/// The key to actually emit: the persisted citekey when present, else computed.
fn key_of(item: &CiteItem) -> String {
    item.citekey
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| citekey(item))
}

fn authors_bibtex(item: &CiteItem) -> String {
    item.authors
        .iter()
        .map(|(g, f)| match (f, g) {
            (Some(f), Some(g)) => format!("{f}, {g}"),
            (Some(f), None) => f.clone(),
            (None, Some(g)) => g.clone(),
            _ => String::new(),
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" and ")
}

fn bibtex_entry(item: &CiteItem) -> String {
    let mut lines = vec![format!("@article{{{},", key_of(item))];
    if let Some(t) = &item.title {
        lines.push(format!("  title = {{{t}}},"));
    }
    let a = authors_bibtex(item);
    if !a.is_empty() {
        lines.push(format!("  author = {{{a}}},"));
    }
    if let Some(y) = item.year {
        lines.push(format!("  year = {{{y}}},"));
    }
    if let Some(v) = &item.venue {
        lines.push(format!("  journal = {{{v}}},"));
    }
    if let Some(d) = &item.doi {
        lines.push(format!("  doi = {{{d}}},"));
    }
    // Drop trailing comma on the last field.
    if let Some(last) = lines.last_mut() {
        if last.ends_with(',') {
            last.pop();
        }
    }
    lines.push("}".to_string());
    lines.join("\n")
}

fn ris_entry(item: &CiteItem) -> String {
    let mut lines = vec!["TY  - JOUR".to_string()];
    if let Some(t) = &item.title {
        lines.push(format!("TI  - {t}"));
    }
    for (g, f) in &item.authors {
        let name = match (f, g) {
            (Some(f), Some(g)) => format!("{f}, {g}"),
            (Some(f), None) => f.clone(),
            (None, Some(g)) => g.clone(),
            _ => String::new(),
        };
        if !name.is_empty() {
            lines.push(format!("AU  - {name}"));
        }
    }
    if let Some(y) = item.year {
        lines.push(format!("PY  - {y}"));
    }
    if let Some(v) = &item.venue {
        lines.push(format!("JO  - {v}"));
    }
    if let Some(d) = &item.doi {
        lines.push(format!("DO  - {d}"));
    }
    lines.push("ER  - ".to_string());
    lines.join("\n")
}

fn csl_json(items: &[CiteItem]) -> String {
    let arr: Vec<_> = items
        .iter()
        .map(|it| {
            json!({
                "id": key_of(it),
                "type": "article-journal",
                "title": it.title,
                "author": it.authors.iter().map(|(g, f)| json!({"given": g, "family": f})).collect::<Vec<_>>(),
                "issued": { "date-parts": [[it.year]] },
                "container-title": it.venue,
                "DOI": it.doi,
            })
        })
        .collect();
    serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".to_string())
}

/// "Given Family" with given reduced to initials for author lists.
fn initials(g: &str) -> String {
    g.split_whitespace()
        .filter_map(|p| p.chars().next())
        .map(|c| format!("{}.", c.to_uppercase()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn apa(item: &CiteItem) -> String {
    let authors = item
        .authors
        .iter()
        .map(|(g, f)| {
            let fam = f.clone().unwrap_or_default();
            match g {
                Some(g) if !g.is_empty() => format!("{fam}, {}", initials(g)),
                _ => fam,
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();
    let auth = match authors.len() {
        0 => String::new(),
        1 => authors[0].clone(),
        _ => format!("{}, & {}", authors[..authors.len() - 1].join(", "), authors[authors.len() - 1]),
    };
    let year = item.year.map(|y| format!("({y}). ")).unwrap_or_default();
    let title = item.title.clone().unwrap_or_default();
    let venue = item.venue.clone().map(|v| format!(" {v}.")).unwrap_or_default();
    let doi = item.doi.clone().map(|d| format!(" https://doi.org/{d}")).unwrap_or_default();
    format!("{auth} {year}{title}.{venue}{doi}").trim().to_string()
}

fn ieee(n: usize, item: &CiteItem) -> String {
    let authors = item
        .authors
        .iter()
        .map(|(g, f)| {
            let fam = f.clone().unwrap_or_default();
            match g {
                Some(g) if !g.is_empty() => format!("{} {fam}", initials(g)),
                _ => fam,
            }
        })
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let title = item.title.clone().unwrap_or_default();
    let venue = item.venue.clone().map(|v| format!(" {v},")).unwrap_or_default();
    let year = item.year.map(|y| format!(" {y}.")).unwrap_or_default();
    let doi = item.doi.clone().map(|d| format!(" doi: {d}.")).unwrap_or_default();
    format!("[{n}] {authors}, \"{title},\"{venue}{year}{doi}")
}

/// Build citation text in the given format/style for all items.
pub fn build(items: &[CiteItem], format: &str) -> String {
    match format {
        "bibtex" => items.iter().map(bibtex_entry).collect::<Vec<_>>().join("\n\n"),
        "ris" => items.iter().map(ris_entry).collect::<Vec<_>>().join("\n\n"),
        "csljson" => csl_json(items),
        "apa" => items.iter().map(apa).collect::<Vec<_>>().join("\n\n"),
        "ieee" => items
            .iter()
            .enumerate()
            .map(|(i, it)| ieee(i + 1, it))
            .collect::<Vec<_>>()
            .join("\n"),
        // Cite-while-write helpers (drop straight into a manuscript).
        "citekey" => items.iter().map(key_of).collect::<Vec<_>>().join("\n"),
        "latex" => {
            let keys = items.iter().map(key_of).collect::<Vec<_>>().join(",");
            if keys.is_empty() {
                String::new()
            } else {
                format!("\\cite{{{keys}}}")
            }
        }
        "pandoc" => {
            let keys = items
                .iter()
                .map(|it| format!("@{}", key_of(it)))
                .collect::<Vec<_>>()
                .join("; ");
            if keys.is_empty() {
                String::new()
            } else {
                format!("[{keys}]")
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> CiteItem {
        CiteItem {
            title: Some("Attention Is All You Need".to_string()),
            authors: vec![
                (Some("Ashish".to_string()), Some("Vaswani".to_string())),
                (Some("Noam".to_string()), Some("Shazeer".to_string())),
            ],
            year: Some(2017),
            venue: Some("NeurIPS".to_string()),
            doi: Some("10.5555/3295222.3295349".to_string()),
            citekey: None,
        }
    }

    #[test]
    fn citekey_and_formats() {
        let it = sample();
        assert_eq!(citekey(&it), "vaswani2017attention");
        assert!(build(&[sample()], "bibtex").contains("@article{vaswani2017attention"));
        assert!(build(&[sample()], "bibtex").contains("author = {Vaswani, Ashish and Shazeer, Noam}"));
        assert!(build(&[sample()], "ris").contains("TY  - JOUR"));
        assert!(build(&[sample()], "csljson").contains("\"family\""));
        assert!(build(&[sample()], "apa").contains("Vaswani, A."));
        assert!(build(&[sample()], "ieee").starts_with("[1]"));
    }

    #[test]
    fn stored_citekey_overrides_computed() {
        // Every key-emitting format uses the stored key verbatim.
        let mut it = sample();
        it.citekey = Some("vaswani2017attentionb".to_string());
        assert!(build(std::slice::from_ref(&it), "bibtex").contains("@article{vaswani2017attentionb"));
        assert!(build(std::slice::from_ref(&it), "csljson").contains("\"id\": \"vaswani2017attentionb\""));
        assert_eq!(build(std::slice::from_ref(&it), "citekey"), "vaswani2017attentionb");
        assert_eq!(build(std::slice::from_ref(&it), "latex"), "\\cite{vaswani2017attentionb}");
        assert_eq!(build(&[it], "pandoc"), "[@vaswani2017attentionb]");
        // ...but a blank/whitespace stored key falls back to the computed one.
        let mut it = sample();
        it.citekey = Some("   ".to_string());
        assert_eq!(build(std::slice::from_ref(&it), "citekey"), "vaswani2017attention");
        assert!(build(&[it], "csljson").contains("\"id\": \"vaswani2017attention\""));
    }
}
