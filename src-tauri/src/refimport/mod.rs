//! Reference-manager import: parse a bibliography exported by Zotero, Mendeley,
//! EndNote, JabRef, Papers, Citavi… into a common [`ParsedRef`] list. Supports
//! **BibTeX/BibLaTeX** (via [`crate::bibtex`]), **RIS**, and **CSL-JSON** — the
//! three formats nearly every reference manager can emit. The command layer turns
//! each `ParsedRef` into a library item (attaching a PDF when one is found, mapping
//! keywords to tags); this module is pure parsing (no DB, no I/O) so it is testable.

/// One bibliographic entry, format-independent. Authors are `(given, family)`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedRef {
    pub title: Option<String>,
    pub doi: Option<String>,
    pub url: Option<String>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    pub abstract_text: Option<String>,
    pub authors: Vec<(Option<String>, Option<String>)>,
    pub keywords: Vec<String>,
    /// Local PDF path hints (BibTeX `file`, RIS `L1`/`L4`, or a `.pdf` URL). May be
    /// absolute, relative to the export folder, or carry only a usable basename.
    pub files: Vec<String>,
    /// Citation key / record id, used only as a fallback synthetic path.
    pub key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefFormat {
    Bibtex,
    Ris,
    CslJson,
}

impl RefFormat {
    pub fn label(self) -> &'static str {
        match self {
            RefFormat::Bibtex => "BibTeX",
            RefFormat::Ris => "RIS",
            RefFormat::CslJson => "CSL-JSON",
        }
    }
}

/// Pick a parser from the file extension, falling back to a content sniff (the file
/// picker also accepts `.txt`, and some managers use non-standard extensions).
pub fn detect_format(ext: Option<&str>, content: &str) -> RefFormat {
    match ext.map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("ris") => RefFormat::Ris,
        Some("json") | Some("csljson") | Some("csl") => RefFormat::CslJson,
        Some("bib") | Some("bibtex") => RefFormat::Bibtex,
        _ => sniff(content),
    }
}

fn sniff(content: &str) -> RefFormat {
    let t = content.trim_start();
    if t.starts_with('[') || t.starts_with('{') {
        return RefFormat::CslJson;
    }
    // RIS records open with a `TY  - <type>` tag line.
    if content
        .lines()
        .take(40)
        .any(|l| ris_tag(l).map(|(t, _)| t == "TY").unwrap_or(false))
    {
        return RefFormat::Ris;
    }
    RefFormat::Bibtex
}

/// Parse `content` in the given format into a list of references.
pub fn parse(content: &str, format: RefFormat) -> Vec<ParsedRef> {
    match format {
        RefFormat::Bibtex => from_bibtex(content),
        RefFormat::Ris => from_ris(content),
        RefFormat::CslJson => from_csl_json(content),
    }
}

// ===== BibTeX =====

fn from_bibtex(content: &str) -> Vec<ParsedRef> {
    crate::bibtex::parse(content)
        .iter()
        .filter_map(bib_entry_to_parsed)
        .collect()
}

fn bib_entry_to_parsed(e: &crate::bibtex::BibEntry) -> Option<ParsedRef> {
    let f = &e.fields;
    let get = |k: &str| f.get(k).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let title = get("title");
    let doi = get("doi");
    let url = get("url");
    let year = get("year").or_else(|| get("date")).and_then(|y| first_year(&y));
    let venue = get("journal").or_else(|| get("booktitle")).or_else(|| get("publisher"));
    let authors = f
        .get("author")
        .map(|a| crate::bibtex::split_authors(a))
        .unwrap_or_default();
    // Zotero uses `keywords`, Mendeley/JabRef `mendeley-tags`/`keywords`.
    let mut keywords = Vec::new();
    for key in ["keywords", "keyword", "mendeley-tags"] {
        if let Some(v) = get(key) {
            keywords.extend(split_keywords(&v));
        }
    }
    dedup_keep_order(&mut keywords);
    let files = get("file").map(|v| parse_bibtex_file_field(&v)).unwrap_or_default();
    if title.is_none() && doi.is_none() {
        return None;
    }
    Some(ParsedRef {
        title,
        doi,
        url,
        year,
        venue,
        abstract_text: get("abstract"),
        authors,
        keywords,
        files,
        key: e.key.clone(),
    })
}

/// Extract PDF path(s) from a BibTeX `file` field. Handles JabRef's
/// `description:path:type` triples (with `\:`/`\\` escaping), Zotero's bare path or
/// `path:application/pdf`, and `;`-separated multiples. Best-effort: even a slightly
/// off path still yields the right basename for an export-folder match.
fn parse_bibtex_file_field(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for chunk in split_unescaped_semicolons(raw) {
        // Unescape JabRef's `\\` and `\:` (order matters: `\\` first).
        let unescaped = chunk.replace("\\\\", "\u{0}").replace("\\:", ":").replace('\u{0}', "\\");
        let mut s = unescaped.trim().to_string();
        // Strip a trailing `:TYPE` (`:PDF`, `:application/pdf`).
        for suf in [":application/pdf", ":pdf"] {
            if s.to_ascii_lowercase().ends_with(suf) {
                s.truncate(s.len() - suf.len());
                break;
            }
        }
        let s = extract_path(s.trim());
        if s.to_ascii_lowercase().ends_with(".pdf") && !s.is_empty() {
            out.push(s);
        }
    }
    out
}

/// Extract the filesystem path from a (possibly `description:path:type`) file entry.
/// Robust to a description that itself contains a colon.
fn extract_path(s: &str) -> String {
    // A Windows drive path anywhere wins (handles descriptions that contain colons).
    if let Some(pos) = drive_pos(s) {
        return s[pos..].trim().to_string();
    }
    // A POSIX absolute path with no leading description.
    if s.starts_with('/') {
        return s.trim().to_string();
    }
    // Otherwise the path is the final `:`-delimited segment (drop the description,
    // even one that contained colons).
    match s.rfind(':') {
        Some(i) => s[i + 1..].trim().to_string(),
        None => s.trim().to_string(),
    }
}

/// Byte index where a `X:\` / `X:/` drive path begins (X a lone drive letter), if any.
fn drive_pos(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    let mut i = 0;
    while i + 2 < b.len() {
        if b[i].is_ascii_alphabetic()
            && b[i + 1] == b':'
            && (b[i + 2] == b'\\' || b[i + 2] == b'/')
            && (i == 0 || !b[i - 1].is_ascii_alphanumeric())
        {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn split_unescaped_semicolons(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            // keep the escape pair intact for later unescaping
            cur.push(c);
            if let Some(&n) = chars.peek() {
                cur.push(n);
                chars.next();
            }
        } else if c == ';' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

// ===== RIS =====

/// A RIS tag line is `XX  - value` (two-char tag, two spaces, dash, space).
fn ris_tag(line: &str) -> Option<(&str, &str)> {
    let b = line.as_bytes();
    // Byte comparisons only — never slice `&line[2..6]`, which would panic if the 3rd
    // byte starts a multibyte char (e.g. a CJK abstract line) and abort the whole import.
    if b.len() >= 6
        && b[0].is_ascii_uppercase()
        && (b[1].is_ascii_uppercase() || b[1].is_ascii_digit())
        && b[2] == b' '
        && b[3] == b' '
        && b[4] == b'-'
        && b[5] == b' '
    {
        // Bytes 0..6 are all ASCII, so byte index 6 is a valid char boundary.
        Some((&line[0..2], line[6..].trim_end()))
    } else {
        None
    }
}

fn from_ris(content: &str) -> Vec<ParsedRef> {
    let mut out = Vec::new();
    let mut cur: Option<RisRecord> = None;
    let mut last_tag = String::new();
    for line in content.lines() {
        match ris_tag(line) {
            Some(("TY", ty)) => {
                if let Some(r) = cur.take() {
                    if let Some(p) = r.into_parsed() {
                        out.push(p);
                    }
                }
                let mut r = RisRecord::default();
                r.ty = ty.to_string();
                cur = Some(r);
                last_tag = "TY".to_string();
            }
            Some(("ER", _)) => {
                if let Some(r) = cur.take() {
                    if let Some(p) = r.into_parsed() {
                        out.push(p);
                    }
                }
                last_tag.clear();
            }
            Some((tag, val)) => {
                if let Some(r) = cur.as_mut() {
                    r.add(tag, val);
                    last_tag = tag.to_string();
                }
            }
            None => {
                // Continuation of the previous tag's value (e.g. a wrapped abstract).
                let t = line.trim();
                if !t.is_empty() {
                    if let Some(r) = cur.as_mut() {
                        r.append_continuation(&last_tag, t);
                    }
                }
            }
        }
    }
    if let Some(r) = cur.take() {
        if let Some(p) = r.into_parsed() {
            out.push(p);
        }
    }
    out
}

#[derive(Default)]
struct RisRecord {
    ty: String,
    title: Option<String>,
    doi: Option<String>,
    url: Option<String>,
    year: Option<String>,
    venue: Option<String>,
    abstract_text: Option<String>,
    id: Option<String>,
    authors: Vec<String>,
    keywords: Vec<String>,
    files: Vec<String>,
    urls: Vec<String>,
}

impl RisRecord {
    fn add(&mut self, tag: &str, val: &str) {
        let v = val.trim();
        if v.is_empty() {
            return;
        }
        match tag {
            "TI" | "T1" => self.title.get_or_insert_with(|| v.to_string()),
            "DO" => self.doi.get_or_insert_with(|| v.to_string()),
            "PY" | "Y1" | "DA" => self.year.get_or_insert_with(|| v.to_string()),
            "JO" | "JF" | "JA" | "T2" => self.venue.get_or_insert_with(|| v.to_string()),
            "AB" | "N2" => self.abstract_text.get_or_insert_with(|| v.to_string()),
            "ID" => self.id.get_or_insert_with(|| v.to_string()),
            "AU" | "A1" | "A2" | "A3" | "A4" => {
                self.authors.push(v.to_string());
                return;
            }
            "KW" => {
                self.keywords.extend(split_keywords(v));
                return;
            }
            "UR" => {
                self.url.get_or_insert_with(|| v.to_string());
                self.urls.push(v.to_string());
                return;
            }
            "L1" | "L4" => {
                self.files.push(v.to_string());
                return;
            }
            _ => return,
        };
    }

    fn append_continuation(&mut self, last_tag: &str, extra: &str) {
        let target = match last_tag {
            "AB" | "N2" => self.abstract_text.as_mut(),
            "TI" | "T1" => self.title.as_mut(),
            _ => None,
        };
        if let Some(s) = target {
            s.push(' ');
            s.push_str(extra);
        }
    }

    fn into_parsed(mut self) -> Option<ParsedRef> {
        let title = self.title.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let doi = self.doi;
        if title.is_none() && doi.is_none() {
            return None;
        }
        let year = self.year.and_then(|y| first_year(&y));
        let authors = self
            .authors
            .iter()
            .map(|a| split_lastname_first(a))
            .filter(|(g, f)| g.is_some() || f.is_some())
            .collect();
        // A `.pdf` link in UR is also a file candidate.
        for u in &self.urls {
            if u.to_ascii_lowercase().ends_with(".pdf") {
                self.files.push(u.clone());
            }
        }
        dedup_keep_order(&mut self.keywords);
        dedup_keep_order(&mut self.files);
        Some(ParsedRef {
            title,
            doi,
            url: self.url,
            year,
            venue: self.venue,
            abstract_text: self.abstract_text,
            authors,
            keywords: self.keywords,
            files: self.files,
            key: self.id.unwrap_or_default(),
        })
    }
}

// ===== CSL-JSON =====

fn from_csl_json(content: &str) -> Vec<ParsedRef> {
    let value: serde_json::Value = match serde_json::from_str(content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let items: Vec<serde_json::Value> = match value {
        serde_json::Value::Array(a) => a,
        // A single item, or a wrapper `{ "items": [...] }` (some tools).
        serde_json::Value::Object(ref o) => o
            .get("items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_else(|| vec![value.clone()]),
        _ => return Vec::new(),
    };
    items.iter().filter_map(csl_item_to_parsed).collect()
}

fn csl_item_to_parsed(it: &serde_json::Value) -> Option<ParsedRef> {
    let s = |k: &str| it.get(k).and_then(|v| v.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let title = s("title");
    let doi = s("DOI").or_else(|| s("doi"));
    if title.is_none() && doi.is_none() {
        return None;
    }
    let year = it
        .get("issued")
        .and_then(|i| i.get("date-parts"))
        .and_then(|d| d.get(0))
        .and_then(|p| p.get(0))
        .and_then(|y| y.as_i64().or_else(|| y.as_str().and_then(|s| first_year(s))))
        .or_else(|| {
            it.get("issued")
                .and_then(|i| i.get("raw"))
                .and_then(|r| r.as_str())
                .and_then(first_year)
        });
    let venue = s("container-title").or_else(|| s("collection-title"));
    let authors = it
        .get("author")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .map(|p| {
                    let given = p.get("given").and_then(|v| v.as_str()).map(str::to_string);
                    let family = p
                        .get("family")
                        .and_then(|v| v.as_str())
                        .or_else(|| p.get("literal").and_then(|v| v.as_str()))
                        .map(str::to_string);
                    (nonempty(given), nonempty(family))
                })
                .filter(|(g, f)| g.is_some() || f.is_some())
                .collect()
        })
        .unwrap_or_default();
    // CSL keywords: a comma-separated string `keyword`, or an array `keywords`.
    let mut keywords = Vec::new();
    if let Some(k) = s("keyword") {
        keywords.extend(split_keywords(&k));
    }
    if let Some(arr) = it.get("keywords").and_then(|v| v.as_array()) {
        for v in arr {
            if let Some(t) = v.as_str() {
                keywords.extend(split_keywords(t));
            }
        }
    }
    dedup_keep_order(&mut keywords);
    Some(ParsedRef {
        title,
        doi,
        url: s("URL").or_else(|| s("url")),
        year,
        venue,
        abstract_text: s("abstract"),
        authors,
        keywords,
        files: Vec::new(), // CSL-JSON carries no file paths
        key: s("id").unwrap_or_default(),
    })
}

// ===== shared helpers =====

fn first_year(s: &str) -> Option<i64> {
    // Find the first run of exactly-4 digits that looks like a year.
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if bytes[i].is_ascii_digit() {
            let run_end = (i..bytes.len()).take_while(|&j| bytes[j].is_ascii_digit()).count() + i;
            if run_end - i >= 4 {
                if let Ok(y) = s[i..i + 4].parse::<i64>() {
                    if (1000..=3000).contains(&y) {
                        return Some(y);
                    }
                }
            }
            i = run_end;
        } else {
            i += 1;
        }
    }
    None
}

/// Split a "Family, Given" (RIS/EndNote) name into `(given, family)`. A trailing
/// suffix ("Family, Given, Jr") is dropped rather than folded into the given name.
fn split_lastname_first(name: &str) -> (Option<String>, Option<String>) {
    let name = name.trim();
    let mut parts = name.splitn(3, ',');
    let family = parts.next().map(str::trim).filter(|s| !s.is_empty());
    let given = parts.next().map(str::trim).filter(|s| !s.is_empty());
    match family {
        Some(f) => (given.map(str::to_string), Some(f.to_string())),
        None => (None, nonempty(Some(name.to_string()))),
    }
}

fn split_keywords(v: &str) -> Vec<String> {
    v.split(|c| c == ',' || c == ';')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn nonempty(s: Option<String>) -> Option<String> {
    s.map(|t| t.trim().to_string()).filter(|t| !t.is_empty())
}

fn dedup_keep_order(v: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    v.retain(|s| seen.insert(s.to_lowercase()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_formats() {
        assert_eq!(detect_format(Some("ris"), ""), RefFormat::Ris);
        assert_eq!(detect_format(Some("json"), "[]"), RefFormat::CslJson);
        assert_eq!(detect_format(Some("bib"), ""), RefFormat::Bibtex);
        assert_eq!(detect_format(None, "  [ {\"id\":1} ]"), RefFormat::CslJson);
        assert_eq!(detect_format(None, "TY  - JOUR\nTI  - X\nER  - "), RefFormat::Ris);
        assert_eq!(detect_format(Some("txt"), "@article{k, title={T}}"), RefFormat::Bibtex);
    }

    #[test]
    fn parses_ris_record() {
        let ris = "TY  - JOUR\r\nTI  - Attention Is All You Need\r\nAU  - Vaswani, Ashish\r\nAU  - Shazeer, Noam\r\nPY  - 2017\r\nJO  - NeurIPS\r\nDO  - 10.5555/ABC\r\nKW  - transformers\r\nKW  - attention\r\nAB  - We propose the\r\nTransformer.\r\nL1  - C:\\lib\\vaswani.pdf\r\nER  - \r\n";
        let refs = parse(ris, RefFormat::Ris);
        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.title.as_deref(), Some("Attention Is All You Need"));
        assert_eq!(r.doi.as_deref(), Some("10.5555/ABC"));
        assert_eq!(r.year, Some(2017));
        assert_eq!(r.venue.as_deref(), Some("NeurIPS"));
        assert_eq!(r.authors.len(), 2);
        assert_eq!(r.authors[0], (Some("Ashish".into()), Some("Vaswani".into())));
        assert_eq!(r.keywords, vec!["transformers", "attention"]);
        assert_eq!(r.abstract_text.as_deref(), Some("We propose the Transformer."));
        assert_eq!(r.files, vec!["C:\\lib\\vaswani.pdf"]);
    }

    #[test]
    fn parses_csl_json() {
        let csl = r#"[{"id":"vaswani2017","type":"article-journal","title":"Attention Is All You Need","DOI":"10.5555/abc","container-title":"NeurIPS","issued":{"date-parts":[[2017,6]]},"author":[{"family":"Vaswani","given":"Ashish"},{"literal":"OpenAI"}],"keyword":"transformers, attention","abstract":"We propose the Transformer."}]"#;
        let refs = parse(csl, RefFormat::CslJson);
        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.title.as_deref(), Some("Attention Is All You Need"));
        assert_eq!(r.doi.as_deref(), Some("10.5555/abc"));
        assert_eq!(r.year, Some(2017));
        assert_eq!(r.venue.as_deref(), Some("NeurIPS"));
        assert_eq!(r.authors.len(), 2);
        assert_eq!(r.authors[0], (Some("Ashish".into()), Some("Vaswani".into())));
        assert_eq!(r.authors[1], (None, Some("OpenAI".into())));
        assert_eq!(r.keywords, vec!["transformers", "attention"]);
    }

    #[test]
    fn parses_bibtex_via_common_path() {
        let bib = "@article{k, title={A Study}, author={Doe, Jane and Roe, Rob}, year={2020}, journal={J}, doi={10.1/X}, keywords={ml; nlp}, file={:C:\\lib\\a.pdf:PDF}}";
        let refs = parse(bib, RefFormat::Bibtex);
        assert_eq!(refs.len(), 1);
        let r = &refs[0];
        assert_eq!(r.title.as_deref(), Some("A Study"));
        assert_eq!(r.doi.as_deref(), Some("10.1/X"));
        assert_eq!(r.year, Some(2020));
        assert_eq!(r.authors.len(), 2);
        assert_eq!(r.keywords, vec!["ml", "nlp"]);
        assert_eq!(r.files, vec!["C:\\lib\\a.pdf"]);
    }

    #[test]
    fn bibtex_file_field_variants() {
        // JabRef desc:path:type with escaped drive colon + backslashes.
        assert_eq!(
            parse_bibtex_file_field("Paper Title:C\\:\\\\lib\\\\a.pdf:PDF"),
            vec!["C:\\lib\\a.pdf".to_string()]
        );
        // Zotero bare absolute path.
        assert_eq!(parse_bibtex_file_field("C:\\Zotero\\storage\\AB\\p.pdf"), vec!["C:\\Zotero\\storage\\AB\\p.pdf".to_string()]);
        // Leading empty description.
        assert_eq!(parse_bibtex_file_field(":/home/me/p.pdf:application/pdf"), vec!["/home/me/p.pdf".to_string()]);
        // Multiple, semicolon-separated; a non-pdf attachment is dropped.
        assert_eq!(
            parse_bibtex_file_field("/a.pdf;note.txt:x:TXT;/b.pdf"),
            vec!["/a.pdf".to_string(), "/b.pdf".to_string()]
        );
    }

    #[test]
    fn skips_entries_without_title_or_doi() {
        assert!(parse("TY  - JOUR\nAU  - Nobody\nER  - ", RefFormat::Ris).is_empty());
        assert!(parse("[{\"type\":\"book\",\"author\":[{\"family\":\"X\"}]}]", RefFormat::CslJson).is_empty());
    }

    #[test]
    fn first_year_extracts_from_dates() {
        assert_eq!(first_year("2017/06/12"), Some(2017));
        assert_eq!(first_year("2017"), Some(2017));
        assert_eq!(first_year("June 2017"), Some(2017));
        assert_eq!(first_year("n.d."), None);
    }

    #[test]
    fn ris_multibyte_does_not_panic() {
        // A CJK title/abstract must not panic ris_tag's byte handling, and a
        // continuation line that starts with a multibyte char must be tolerated.
        let ris = "TY  - JOUR\nTI  - 日本語のタイトル\nAB  - 概要のテキスト\n続きの行\nDO  - 10.1/x\nER  - ";
        let refs = parse(ris, RefFormat::Ris);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].title.as_deref(), Some("日本語のタイトル"));
        assert_eq!(refs[0].abstract_text.as_deref(), Some("概要のテキスト 続きの行"));
    }

    #[test]
    fn bibtex_file_field_description_with_colon() {
        // A JabRef description that itself contains a colon must not corrupt the path.
        assert_eq!(
            parse_bibtex_file_field("Notes: draft:C\\:\\\\lib\\\\a.pdf:PDF"),
            vec!["C:\\lib\\a.pdf".to_string()]
        );
        // POSIX path behind a colon-bearing description.
        assert_eq!(
            parse_bibtex_file_field("Draft version:/home/me/p.pdf:application/pdf"),
            vec!["/home/me/p.pdf".to_string()]
        );
    }

    #[test]
    fn ris_author_suffix_dropped() {
        let ris = "TY  - JOUR\nTI  - X\nAU  - Smith, John, Jr\nER  - ";
        let refs = parse(ris, RefFormat::Ris);
        assert_eq!(refs[0].authors[0], (Some("John".into()), Some("Smith".into())));
    }
}
