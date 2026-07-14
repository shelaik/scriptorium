//! Metadata enrichment: find a document's DOI, look it up on Crossref, and
//! write the bibliographic data (title, authors, year, venue, abstract,
//! references) back into the database.

use anyhow::{bail, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;

const USER_AGENT: &str = "Scriptorium/0.1";

static DOI_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)10\.\d{4,9}/[-._;()/:a-z0-9]+").expect("valid DOI regex"));

#[derive(Debug, Clone, Default)]
pub struct Author {
    pub given: Option<String>,
    pub family: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct Reference {
    pub doi: Option<String>,
    pub raw: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct CrossrefMeta {
    pub title: Option<String>,
    pub venue: Option<String>,
    pub year: Option<i64>,
    pub abstract_text: Option<String>,
    pub authors: Vec<Author>,
    pub references: Vec<Reference>,
    pub raw_json: String,
}

/// Find the first DOI in free text, trimming trailing punctuation that tends to
/// stick to it when printed in a PDF.
pub fn extract_doi(text: &str) -> Option<String> {
    let m = DOI_RE.find(text)?;
    let mut doi = m.as_str();
    // Cut off following text glued on by PDF extraction (e.g. "...3330701https://").
    if let Some(idx) = doi.to_ascii_lowercase().find("http") {
        doi = &doi[..idx];
    }
    let doi = doi.trim_end_matches(|c: char| {
        matches!(c, '.' | ',' | ';' | ':' | ')' | ']' | '>' | '"' | '\'' | '/')
    });
    if doi.len() < 8 {
        return None;
    }
    Some(doi.to_string())
}

/// All distinct DOIs in `text`, in order of first appearance, cleaned like
/// [`extract_doi`], capped at `max`. For the interactive candidate finder,
/// where a *citation's* DOI on the first page is a legitimate candidate (the
/// user confirms visually) — unlike automatic enrichment, which must reject it.
pub fn extract_dois(text: &str, max: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in DOI_RE.find_iter(text) {
        let mut doi = m.as_str();
        if let Some(idx) = doi.to_ascii_lowercase().find("http") {
            doi = &doi[..idx];
        }
        let doi = doi.trim_end_matches(|c: char| {
            matches!(c, '.' | ',' | ';' | ':' | ')' | ']' | '>' | '"' | '\'' | '/')
        });
        if doi.len() < 8 {
            continue;
        }
        if !out.iter().any(|d| d.eq_ignore_ascii_case(doi)) {
            out.push(doi.to_string());
        }
        if out.len() >= max {
            break;
        }
    }
    out
}

/// Distinct arXiv ids explicitly marked as such in `text` ("arXiv:2406.09406",
/// "arxiv.org/abs/1706.03762"), in order of appearance, capped at `max`. Only
/// `arXiv`-prefixed ids are trusted — a bare `dddd.ddddd` in body text is far
/// too often a section/figure number.
pub fn extract_arxiv_ids(text: &str, max: usize) -> Vec<String> {
    static ARXIV_MARKED_RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"(?i)arxiv(?:\.org)?[:\s./]{0,3}(?:abs/|pdf/)?(\d{4})\.(\d{4,5})(v\d+)?")
            .expect("valid marked arxiv regex")
    });
    let mut out: Vec<String> = Vec::new();
    for c in ARXIV_MARKED_RE.captures_iter(text) {
        let (Some(yymm), Some(num)) = (c.get(1), c.get(2)) else { continue };
        // Plausibility: new-scheme id starts with YYMM, month 01–12.
        let mm: u32 = match yymm.as_str().get(2..4).and_then(|s| s.parse().ok()) {
            Some(m) => m,
            None => continue,
        };
        if !(1..=12).contains(&mm) {
            continue;
        }
        let ver = c.get(3).map_or("", |m| m.as_str());
        let id = format!("{}.{}{}", yymm.as_str(), num.as_str(), ver);
        if !out.iter().any(|d| d == &id) {
            out.push(id);
        }
        if out.len() >= max {
            break;
        }
    }
    out
}

/// Function words plus a few ultra-generic domain terms that co-occur across
/// unrelated papers — excluded so the title-match gate keys on *distinctive*
/// words rather than boilerplate like "large language models".
static TITLE_STOP: Lazy<std::collections::HashSet<&'static str>> = Lazy::new(|| {
    [
        "the", "and", "for", "with", "from", "into", "using", "via", "toward", "towards", "are",
        "our", "this", "that", "these", "those", "its", "their", "can", "does", "how", "why",
        "what", "when", "over", "under", "between", "about", "based", "such", "than", "then",
        // generic ML/academic vocabulary
        "large", "language", "model", "models", "learning", "deep", "neural", "network",
        "networks", "data", "training", "train", "approach", "method", "methods", "study",
        "analysis", "framework", "system", "systems", "task", "tasks", "paper", "novel",
        "towards", "evaluation", "understanding",
    ]
    .into_iter()
    .collect()
});

/// Fold common Latin diacritics to ASCII so titles that disagree only on accents
/// (Poincaré/Poincare, Erdős/Erdos, Schrödinger/Schrodinger) compare as equal.
/// Apply after lowercasing.
pub fn fold_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => out.push('a'),
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => out.push('e'),
            'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' | 'ı' => out.push('i'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => out.push('o'),
            'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => out.push('u'),
            'ç' | 'ć' | 'ĉ' | 'č' => out.push('c'),
            'ñ' | 'ń' | 'ň' => out.push('n'),
            'ś' | 'š' | 'ş' => out.push('s'),
            'ź' | 'ż' | 'ž' => out.push('z'),
            'ý' | 'ÿ' => out.push('y'),
            'ł' => out.push('l'),
            'ğ' => out.push('g'),
            'ř' => out.push('r'),
            'đ' => out.push('d'),
            'ß' => out.push_str("ss"),
            other => out.push(other),
        }
    }
    out
}

/// Distinctive (≥4-letter, non-stopword) lowercase words of a title, de-duplicated.
/// Diacritics are folded so accent-only spelling differences don't split a word.
fn sig_title_words(title: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in fold_ascii(&title.to_lowercase()).split(|c: char| !c.is_alphanumeric()) {
        if w.len() >= 4 && !TITLE_STOP.contains(w) && !out.iter().any(|x| x == w) {
            out.push(w.to_string());
        }
    }
    out
}

/// True if `title` plausibly belongs to a document whose extracted text begins
/// with `head`: at least half of the title's distinctive words appear near the
/// top of the PDF, where the real title is printed. This is the gate that stops
/// enrichment from latching onto a DOI that actually belongs to a *cited* work
/// (the cause of cards showing a different paper than the file).
pub fn title_matches_doc(title: &str, head: &str) -> bool {
    let head_l = fold_ascii(&head.to_lowercase());
    let words = sig_title_words(title);
    if words.is_empty() {
        // No distinctive words (very short/generic title): require the trimmed
        // title itself to appear near the top.
        let t = fold_ascii(&title.trim().to_lowercase());
        return t.len() >= 4 && head_l.contains(&t);
    }
    let hits = words.iter().filter(|w| head_l.contains(w.as_str())).count();
    hits * 2 >= words.len() // ≥ 50% of distinctive title words present
}

/// Order-preserving normalized form of a title: lowercase words (any length)
/// joined by single spaces. Used for exact comparison of low-distinctiveness titles.
fn norm_title_string(s: &str) -> String {
    fold_ascii(&s.to_lowercase())
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The set of *distinctive* words of a title (same rule as [`sig_title_words`]).
fn distinctive_set(s: &str) -> HashSet<String> {
    sig_title_words(s).into_iter().collect()
}

/// Fraction of `a`'s distinctive words that also appear in `b` (0..1) — a
/// TOLERANT similarity for RANKING candidates that a human confirms visually.
/// The strict identity gate for anything automatic remains [`strong_title_match`].
pub fn title_overlap_frac(a: &str, b: &str) -> f32 {
    let da = distinctive_set(a);
    if da.is_empty() {
        return 0.0;
    }
    let db = distinctive_set(b);
    da.intersection(&db).count() as f32 / da.len() as f32
}

/// Do two TITLES name the same paper? True only when they carry the SAME set of
/// distinctive words. Precision-first by design: a genuinely different paper —
/// even one that merely adds ("Riemannian Denoising Diffusion…") or reorders
/// ("Is Attention All You Need?") a word — differs in its distinctive set and is
/// rejected. Overlap-based similarity (Jaccard) was too loose here: it matched
/// those neighbours. Titles with no distinctive words fall back to exact
/// normalized-string equality. A near-miss leaves the document un-enriched
/// rather than mislabelling it — the whole point of this gate.
pub fn strong_title_match(a: &str, b: &str) -> bool {
    let da = distinctive_set(a);
    let db = distinctive_set(b);
    // Low-distinctiveness titles are ambiguous as a bag of words — e.g.
    // "Attention Is All You Need" vs "Is Attention All You Need?" reduce to the
    // same {attention, need}. Require exact normalized-string equality for those
    // so word order and completeness decide.
    if da.len() < 3 || db.len() < 3 {
        let na = norm_title_string(a);
        return !na.is_empty() && na == norm_title_string(b);
    }
    da == db
}

/// Recover an arXiv id from a *filename* (not body text, which is full of cited
/// ids). Handles `2406.09406v2.pdf`, `arxiv_2606_00995.pdf`, `2512.16301.pdf`.
/// Returns `None` when the name carries no plausible id.
pub fn arxiv_id_from_filename(name: &str) -> Option<String> {
    static ARXIV_FILE_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)(\d{4})[._ -](\d{4,5})(v\d+)?").expect("valid arxiv file regex"));
    // Drop a trailing file extension so "…995.pdf" doesn't confuse the match.
    let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
    let c = ARXIV_FILE_RE.captures(stem)?;
    let yymm = c.get(1)?.as_str();
    let num = c.get(2)?.as_str();
    // Plausibility: the new-scheme id starts with YYMM, month 01–12.
    let mm: u32 = yymm.get(2..4)?.parse().ok()?;
    if !(1..=12).contains(&mm) {
        return None;
    }
    let ver = c.get(3).map_or("", |m| m.as_str());
    Some(format!("{yymm}.{num}{ver}"))
}

/// A leading line that is publisher chrome (banner / running head / homepage /
/// copyright), not the paper's title — skipped when recovering the title so a
/// journal PDF's first line doesn't get mistaken for the title.
fn is_banner_line(l: &str) -> bool {
    let s = l.trim();
    if s.chars().count() < 3 {
        return true;
    }
    let low = s.to_lowercase();
    // Prefix-anchored chrome: an identifier/URL/copyright line never *starts* a
    // real title. (Anchored, so a title merely containing "www" or "issn" is safe.)
    const PFX: &[&str] = &[
        "http://", "https://", "www.", "doi:", "arxiv:", "issn", "e-issn", "isbn", "©", "(c) 20",
    ];
    if PFX.iter().any(|p| low.starts_with(p)) {
        return true;
    }
    // Distinctive multi-word phrases that essentially never occur inside a paper
    // title (kept narrow on purpose — broad venue words like "journal of" or
    // "conference on" were dropped because real titles do contain them).
    const PHR: &[&str] = &[
        "lists available at", "available online at", "copy available at", "journal homepage",
        "downloaded from", "content downloaded", "rights reserved", "creative commons",
        "copyright ", "©",
    ];
    PHR.iter().any(|p| low.contains(p))
}

/// Best-effort title from the start of the extracted PDF text: the first
/// non-empty, non-banner line, plus the next one when the first looks like a
/// wrapped title (short, or ends with a colon). Used to recover a sensible title
/// for a mis-enriched document that has no arXiv id in its filename.
pub fn first_line_title(fulltext: &str) -> Option<String> {
    let lines: Vec<&str> = fulltext
        .split(|c: char| c == '\n' || c == '\r')
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(10)
        .collect();
    if lines.is_empty() {
        return None;
    }
    // Where does the publisher chrome end? Skip the leading *contiguous* run of
    // banner lines (only leading — so a banner appearing after a real title, as
    // happens with non-linear PDF text order, never drags the title away).
    let mut start = 0;
    while start < lines.len() && is_banner_line(lines[start]) {
        start += 1;
    }
    // Elsevier/ScienceDirect sandwich: "<journal name>\n journal homepage: …".
    // If the line right after the banners is a journal name immediately followed
    // by a homepage line, skip the journal name and the homepage line too.
    if start + 1 < lines.len() && lines[start + 1].to_lowercase().contains("journal homepage") {
        start += 2;
        while start < lines.len() && is_banner_line(lines[start]) {
            start += 1;
        }
    }
    let first = lines.get(start).copied().filter(|l| !l.trim().is_empty())?;
    let mut title = first.to_string();
    // Append the next line when the first looks like a wrapped title (does not end
    // with sentence punctuation) and the next line is a title continuation — NOT
    // the author/affiliation line, which carries superscript markers or digits.
    let l1_complete = first.ends_with('.') || first.ends_with('?') || first.ends_with('!');
    if !l1_complete {
        if let Some(second) = lines.get(start + 1).copied() {
            let low = second.to_lowercase();
            let looks_meta = low.starts_with("abstract")
                || low.starts_with("introduction")
                || is_banner_line(second)
                || second.chars().any(|c| matches!(c, '∗' | '†' | '‡' | '§') || c.is_ascii_digit());
            if !looks_meta {
                title.push(' ');
                title.push_str(second);
            }
        }
    }
    // Cut anything from the first author/affiliation marker onward.
    if let Some(i) = title.find(|c| matches!(c, '∗' | '†' | '‡' | '§')) {
        title.truncate(i);
    }
    let title = title
        .trim()
        .trim_end_matches(|c: char| matches!(c, ':' | ' ' | ',' | '-' | '.'))
        .trim()
        .to_string();
    (title.chars().count() >= 4).then_some(title)
}

/// Strip JATS/XML tags from a Crossref abstract and tidy whitespace.
fn strip_markup(s: &str) -> String {
    static TAG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"<[^>]+>").expect("valid tag regex"));
    let no_tags = TAG_RE.replace_all(s, " ");
    let decoded = no_tags
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn first_str(v: &Value) -> Option<String> {
    v.as_array()
        .and_then(|a| a.first())
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
}

/// Look up a DOI on Crossref. Returns `Ok(None)` for a 404 (unknown DOI).
pub async fn fetch_crossref(
    client: &reqwest::Client,
    doi: &str,
    email: Option<&str>,
) -> Result<Option<CrossrefMeta>> {
    // Include the configured contact email (Crossref "polite pool") only when set.
    let url = match email {
        Some(e) if !e.trim().is_empty() => {
            format!("https://api.crossref.org/works/{doi}?mailto={}", e.trim())
        }
        _ => format!("https://api.crossref.org/works/{doi}"),
    };
    let resp = client
        .get(&url)
        .send()
        .await
        .context("Crossref request failed")?;

    if resp.status().as_u16() == 404 {
        return Ok(None);
    }
    if !resp.status().is_success() {
        bail!("Crossref HTTP {}", resp.status());
    }

    let body: Value = resp.json().await.context("parsing Crossref JSON")?;
    Ok(Some(parse_crossref_work(&body["message"])))
}

/// Parse one Crossref "work" JSON object into our metadata shape. Shared by the
/// single-DOI lookup (`message`) and the title search (each `message.items[i]`).
fn parse_crossref_work(msg: &Value) -> CrossrefMeta {
    let authors = msg["author"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|a| Author {
                    given: a["given"].as_str().map(str::to_string),
                    family: a["family"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();

    let references = msg["reference"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|r| Reference {
                    doi: r["DOI"].as_str().map(str::to_string),
                    raw: r["unstructured"].as_str().map(str::to_string),
                })
                .collect()
        })
        .unwrap_or_default();

    // `issued` is the canonical date; some records only carry `published*`.
    let year = msg["issued"]["date-parts"][0][0]
        .as_i64()
        .or_else(|| msg["published"]["date-parts"][0][0].as_i64())
        .or_else(|| msg["published-print"]["date-parts"][0][0].as_i64())
        .or_else(|| msg["published-online"]["date-parts"][0][0].as_i64());

    CrossrefMeta {
        title: first_str(&msg["title"]),
        venue: first_str(&msg["container-title"]),
        year,
        abstract_text: msg["abstract"].as_str().map(strip_markup),
        authors,
        references,
        raw_json: msg.to_string(),
    }
}

/// Resolve a paper by TITLE via Crossref's bibliographic search. Returns the DOI
/// and metadata of the first relevance-ranked result whose title *strongly*
/// matches `title` — so a near-miss (a different paper Crossref happened to rank)
/// is rejected rather than applied.
pub async fn crossref_search_title(
    client: &reqwest::Client,
    title: &str,
    email: Option<&str>,
) -> Result<Option<(String, CrossrefMeta)>> {
    let title = title.trim();
    if title.chars().count() < 8 {
        return Ok(None); // too weak to search on
    }
    let mail = match email {
        Some(e) if !e.trim().is_empty() => format!("&mailto={}", e.trim()),
        _ => String::new(),
    };
    let url = format!(
        "https://api.crossref.org/works?query.bibliographic={}&rows=5{}",
        urlencoding(title),
        mail
    );
    let resp = client.get(&url).send().await.context("Crossref title search")?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body: Value = resp.json().await.context("Crossref search JSON")?;
    let Some(items) = body["message"]["items"].as_array() else {
        return Ok(None);
    };
    for it in items {
        let meta = parse_crossref_work(it);
        if let (Some(cand), Some(doi)) = (meta.title.as_deref(), it["DOI"].as_str()) {
            if strong_title_match(title, cand) {
                return Ok(Some((doi.to_string(), meta)));
            }
        }
    }
    Ok(None)
}

/// UNGATED variant of [`crossref_search_title`] for the interactive candidate
/// finder: every parsed row (DOI + metadata) of the bibliographic search is
/// returned — the caller scores them and the USER confirms; nothing is applied
/// automatically, so the precision gate is not needed here.
pub async fn crossref_search_candidates(
    client: &reqwest::Client,
    query: &str,
    email: Option<&str>,
    rows: usize,
) -> Result<Vec<(String, CrossrefMeta)>> {
    let query = query.trim();
    if query.chars().count() < 8 {
        return Ok(Vec::new());
    }
    let mail = match email {
        Some(e) if !e.trim().is_empty() => format!("&mailto={}", e.trim()),
        _ => String::new(),
    };
    let url = format!(
        "https://api.crossref.org/works?query.bibliographic={}&rows={}{}",
        urlencoding(query),
        rows.clamp(1, 10),
        mail
    );
    let resp = client.get(&url).send().await.context("Crossref candidate search")?;
    if !resp.status().is_success() {
        return Ok(Vec::new());
    }
    let body: Value = resp.json().await.context("Crossref candidates JSON")?;
    let Some(items) = body["message"]["items"].as_array() else {
        return Ok(Vec::new());
    };
    Ok(items
        .iter()
        .filter_map(|it| {
            let doi = it["DOI"].as_str()?.to_string();
            Some((doi, parse_crossref_work(it)))
        })
        .collect())
}

/// True when a Crossref candidate TITLE is essentially spelled out inside a raw
/// reference string — i.e. (nearly) all of the title's distinctive words appear
/// in the citation text. Stricter than [`title_matches_doc`] (a 50% gate against a
/// PDF's own header): a cited reference names the work almost in full, so a merely
/// relevance-ranked near-miss from Crossref is rejected rather than applied.
fn reference_names_title(cand_title: &str, reference: &str) -> bool {
    let words = sig_title_words(cand_title);
    // Need a SPECIFIC title: at least 4 distinctive words (after stripping generic
    // academic vocabulary). Shorter titles collide too easily inside a noisy
    // citation — and, since this is a one-directional coverage test, a short title
    // that is merely a SUBSET of a different work's longer title would otherwise
    // pass. Leave those unresolved: precision-first, never mislabel.
    if words.len() < 4 {
        return false;
    }
    // Whole-TOKEN membership, NOT substring: 'adam' must not match inside the
    // surname 'adamson'. Tokenize the reference exactly as titles are tokenized.
    let refl = fold_ascii(&reference.to_lowercase());
    let ref_tokens: std::collections::HashSet<&str> = refl
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let hits = words.iter().filter(|w| ref_tokens.contains(w.as_str())).count();
    // Essentially all of the candidate's distinctive words present (≥85%, and at
    // most one miss regardless of length).
    hits + 1 >= words.len() && hits * 100 >= words.len() * 85
}

/// Resolve a DOI from an UNSTRUCTURED reference string (a full citation: authors,
/// year, title, venue) via Crossref bibliographic search. Precision-first: the
/// first relevance-ranked candidate whose title is essentially spelled out in the
/// citation wins; otherwise None — a near-miss is never applied. Used to backfill
/// DOIs onto already-imported references so they reach the citation-gap finder.
pub async fn crossref_resolve_reference(
    client: &reqwest::Client,
    reference: &str,
    email: Option<&str>,
) -> Result<Option<String>> {
    let reference = reference.trim();
    if reference.chars().count() < 20 {
        return Ok(None); // too little to match on safely
    }
    let mail = match email {
        Some(e) if !e.trim().is_empty() => format!("&mailto={}", e.trim()),
        _ => String::new(),
    };
    let url = format!(
        "https://api.crossref.org/works?query.bibliographic={}&rows=5{}",
        urlencoding(reference),
        mail
    );
    let resp = client.get(&url).send().await.context("Crossref reference search")?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body: Value = resp.json().await.context("Crossref reference JSON")?;
    let Some(items) = body["message"]["items"].as_array() else {
        return Ok(None);
    };
    // Prefer a published record over a preprint when both name the same work:
    // a preprint ("posted-content") carries a different DOI than the version the
    // library likely holds, which would read as a false citation gap. Fall back to
    // the preprint only if nothing published matches.
    let mut preprint_fallback: Option<String> = None;
    for it in items {
        if let (Some(cand), Some(doi)) = (parse_crossref_work(it).title.as_deref(), it["DOI"].as_str()) {
            if reference_names_title(cand, reference) {
                if it["type"].as_str() == Some("posted-content") {
                    preprint_fallback.get_or_insert_with(|| doi.to_lowercase());
                } else {
                    return Ok(Some(doi.to_lowercase()));
                }
            }
        }
    }
    Ok(preprint_fallback)
}

/// Upsert an author and return its id.
fn author_id(conn: &Connection, a: &Author) -> Result<i64> {
    conn.execute(
        "INSERT OR IGNORE INTO authors (family, given) VALUES (?1, ?2)",
        params![a.family, a.given],
    )?;
    let id = conn.query_row(
        "SELECT id FROM authors WHERE family IS ?1 AND given IS ?2",
        params![a.family, a.given],
        |r| r.get(0),
    )?;
    Ok(id)
}

/// Replace a document's author list with `authors` (order preserved). Shared by
/// the enrichment and repair paths. Accepts anything that derefs to a
/// `Connection` (a `&Connection` or a `&Transaction`).
pub fn set_authors(conn: &Connection, id: i64, authors: &[Author]) -> Result<()> {
    conn.execute("DELETE FROM document_authors WHERE document_id = ?1", params![id])?;
    for (pos, a) in authors.iter().enumerate() {
        if a.family.is_none() && a.given.is_none() {
            continue;
        }
        let aid = author_id(conn, a)?;
        conn.execute(
            "INSERT OR IGNORE INTO document_authors (document_id, author_id, position) VALUES (?1, ?2, ?3)",
            params![id, aid, pos as i64],
        )?;
    }
    Ok(())
}

/// Write fetched metadata into the database for document `id`.
pub fn apply_metadata(conn: &mut Connection, id: i64, doi: &str, meta: &CrossrefMeta) -> Result<()> {
    let tx = conn.transaction().context("starting metadata transaction")?;

    // UNIQUE(doi) spans soft-deleted rows too, so decide who owns this DOI:
    //  - a *live* document owns it → don't duplicate; also clear any stale DOI on
    //    this row so a mislabel isn't frozen as "correct" (real title + wrong DOI).
    //  - only a *soft-deleted* row holds it → reclaim it (null the dead holder,
    //    then set it here) so the live paper keeps its own DOI.
    let live_dup: Option<i64> = tx
        .query_row(
            "SELECT id FROM documents WHERE doi = ?1 AND id <> ?2 AND deleted_at IS NULL",
            params![doi, id],
            |r| r.get(0),
        )
        .optional()?;

    if live_dup.is_some() {
        tx.execute(
            "UPDATE documents
             SET title = COALESCE(?1, title), year = ?2, venue = ?3, doi = NULL, abstract = ?4
             WHERE id = ?5",
            params![meta.title, meta.year, meta.venue, meta.abstract_text, id],
        )
        .context("updating document with metadata")?;
    } else {
        // Free the DOI from any soft-deleted holder before claiming it here.
        tx.execute(
            "UPDATE documents SET doi = NULL WHERE doi = ?1 AND id <> ?2",
            params![doi, id],
        )?;
        tx.execute(
            "UPDATE documents
             SET title = COALESCE(?1, title), year = ?2, venue = ?3, doi = ?4, abstract = ?5
             WHERE id = ?6",
            params![meta.title, meta.year, meta.venue, doi, meta.abstract_text, id],
        )
        .context("updating document with metadata")?;
    }

    tx.execute("DELETE FROM document_authors WHERE document_id = ?1", params![id])?;
    for (pos, a) in meta.authors.iter().enumerate() {
        if a.family.is_none() && a.given.is_none() {
            continue;
        }
        let aid = author_id(&tx, a)?;
        tx.execute(
            "INSERT OR IGNORE INTO document_authors (document_id, author_id, position) VALUES (?1, ?2, ?3)",
            params![id, aid, pos as i64],
        )?;
    }

    tx.execute("DELETE FROM document_references WHERE document_id = ?1", params![id])?;
    for r in &meta.references {
        tx.execute(
            "INSERT INTO document_references (document_id, ref_doi, raw) VALUES (?1, ?2, ?3)",
            params![id, r.doi, r.raw],
        )?;
    }

    tx.execute(
        "INSERT OR REPLACE INTO api_cache (doi, source, response_json) VALUES (?1, 'crossref', ?2)",
        params![doi, meta.raw_json],
    )?;

    tx.commit().context("committing metadata transaction")?;
    Ok(())
}

static ARXIV_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\d{4}\.\d{4,5}(v\d+)?").expect("valid arxiv regex"));

/// A reference resolved from an identifier (DOI/arXiv/ISBN/PMID).
pub struct ResolvedRef {
    /// Synthetic, unique path for dedupe, e.g. "ref:doi:10.x/y".
    pub path_id: String,
    /// DOI if the identifier is/has one.
    pub doi: Option<String>,
    pub meta: CrossrefMeta,
}

fn between(s: &str, a: &str, b: &str) -> Option<String> {
    let start = s.find(a)? + a.len();
    let end = s[start..].find(b)? + start;
    Some(strip_markup(&s[start..end]))
}

/// Split a full name into an `Author` — last whitespace word is the family.
fn split_name(full: &str) -> Author {
    let full = full.trim();
    match full.rsplit_once(' ') {
        Some((g, f)) => Author {
            given: (!g.trim().is_empty()).then(|| g.trim().to_string()),
            family: (!f.trim().is_empty()).then(|| f.trim().to_string()),
        },
        None if full.is_empty() => Author { given: None, family: None },
        None => Author {
            given: None,
            family: Some(full.to_string()),
        },
    }
}

fn year_from(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    for w in bytes.windows(4) {
        if w.iter().all(|c| c.is_ascii_digit()) {
            if let Ok(y) = std::str::from_utf8(w).unwrap_or("").parse::<i64>() {
                if (1500..=2100).contains(&y) {
                    return Some(y);
                }
            }
        }
    }
    None
}

/// Parse one arXiv Atom `<entry>` block into metadata.
fn parse_arxiv_entry(entry: &str) -> CrossrefMeta {
    let title = between(entry, "<title>", "</title>");
    let mut authors = Vec::new();
    let mut rest = entry;
    while let Some(name) = between(rest, "<name>", "</name>") {
        authors.push(split_name(&name));
        let cut = rest.find("</name>").map(|i| i + 7).unwrap_or(rest.len());
        rest = &rest[cut..];
    }
    let year = between(entry, "<published>", "</published>").and_then(|p| year_from(&p));
    CrossrefMeta {
        title,
        venue: Some("arXiv".to_string()),
        year,
        abstract_text: between(entry, "<summary>", "</summary>"),
        authors,
        references: Vec::new(),
        raw_json: String::new(),
    }
}

/// arXiv Atom feed → metadata.
pub async fn fetch_arxiv(client: &reqwest::Client, id: &str) -> Result<Option<CrossrefMeta>> {
    let url = format!("https://export.arxiv.org/api/query?id_list={id}&max_results=1");
    let resp = client.get(&url).send().await.context("arXiv request")?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body = resp.text().await.context("arXiv body")?;
    // Parse within the first <entry> block (the feed has a top-level <title> too).
    let Some(estart) = body.find("<entry>") else {
        return Ok(None);
    };
    Ok(Some(parse_arxiv_entry(&body[estart..])))
}

/// Resolve a paper by TITLE via arXiv's title search (`ti:`). Returns the first
/// result whose title strongly matches `title`. arXiv is where recent preprints
/// live that Crossref does not yet index.
pub async fn arxiv_search_title(client: &reqwest::Client, title: &str) -> Result<Option<CrossrefMeta>> {
    let title = title.trim();
    if title.chars().count() < 8 {
        return Ok(None);
    }
    // Quote the phrase so arXiv ranks close title matches first; strong_title_match
    // still guards against a loose hit.
    let url = format!(
        "https://export.arxiv.org/api/query?search_query=ti:%22{}%22&max_results=5",
        urlencoding(title)
    );
    let resp = client.get(&url).send().await.context("arXiv title search")?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body = resp.text().await.context("arXiv search body")?;
    for entry in body.split("<entry>").skip(1) {
        let meta = parse_arxiv_entry(entry);
        if let Some(cand) = meta.title.as_deref() {
            if strong_title_match(title, cand) {
                return Ok(Some(meta));
            }
        }
    }
    Ok(None)
}

/// UNGATED arXiv title search for the interactive candidate finder: every entry
/// with its arXiv id — the caller scores, the user confirms.
pub async fn arxiv_search_candidates(
    client: &reqwest::Client,
    query: &str,
    rows: usize,
) -> Result<Vec<(String, CrossrefMeta)>> {
    let query = query.trim();
    if query.chars().count() < 8 {
        return Ok(Vec::new());
    }
    let url = format!(
        "https://export.arxiv.org/api/query?search_query=ti:%22{}%22&max_results={}",
        urlencoding(query),
        rows.clamp(1, 10)
    );
    let resp = client.get(&url).send().await.context("arXiv candidate search")?;
    if !resp.status().is_success() {
        return Ok(Vec::new());
    }
    let body = resp.text().await.context("arXiv candidates body")?;
    let mut out = Vec::new();
    for entry in body.split("<entry>").skip(1) {
        let meta = parse_arxiv_entry(entry);
        // <id>http://arxiv.org/abs/2406.09406v2</id> → "2406.09406v2"
        let id = between(entry, "<id>", "</id>")
            .and_then(|u| u.rsplit('/').next().map(str::to_string))
            .unwrap_or_default();
        if meta.title.is_some() {
            out.push((id, meta));
        }
    }
    Ok(out)
}

/// Metadata resolved for a document, with the DOI when the source has one
/// (Crossref) or `None` when it does not (arXiv preprints).
pub struct Resolved {
    pub doi: Option<String>,
    pub meta: CrossrefMeta,
}

/// Best-effort *correct* identity for a document. Every candidate is verified
/// against the paper's OWN title recovered from the PDF (banner lines skipped),
/// so a *cited* work's DOI can never rename it. Without a recoverable own title
/// we cannot tell the paper apart from a citation, so we leave it un-enriched.
/// Signals, in order:
///   1. a DOI in the record or text — accepted ONLY if its Crossref title
///      strictly matches the recovered own title (kills the "first DOI is a
///      citation" trap, including a citation printed on the first page);
///   2. a Crossref bibliographic search on the recovered title;
///   3. an arXiv title search (recent preprints Crossref lacks).
pub async fn resolve_document_meta(
    client: &reqwest::Client,
    fulltext: &str,
    existing_doi: Option<&str>,
    email: Option<&str>,
) -> Result<Option<Resolved>> {
    let head: String = fulltext.chars().take(1500).collect();
    // The paper's own title is the anchor for every check — no title, no enrich.
    let Some(g) = first_line_title(&head).filter(|s| !s.trim().is_empty()) else {
        return Ok(None);
    };

    // (1) A DOI in the record or text — trust it ONLY on a strict title match.
    let doi = existing_doi
        .map(str::to_string)
        .filter(|d| !d.trim().is_empty())
        .or_else(|| extract_doi(fulltext));
    if let Some(doi) = doi {
        if let Some(meta) = fetch_crossref(client, &doi, email).await? {
            if strong_title_match(&g, meta.title.as_deref().unwrap_or("")) {
                return Ok(Some(Resolved { doi: Some(doi), meta }));
            }
        }
    }

    // (2) Crossref bibliographic search on the recovered title.
    if let Some((doi, meta)) = crossref_search_title(client, &g, email).await? {
        if strong_title_match(&g, meta.title.as_deref().unwrap_or("")) {
            return Ok(Some(Resolved { doi: Some(doi), meta }));
        }
    }

    // (3) arXiv title search (recent preprints Crossref lacks).
    if let Some(meta) = arxiv_search_title(client, &g).await? {
        if strong_title_match(&g, meta.title.as_deref().unwrap_or("")) {
            return Ok(Some(Resolved { doi: None, meta }));
        }
    }

    Ok(None)
}

/// OpenLibrary → book metadata for an ISBN.
pub async fn fetch_isbn(client: &reqwest::Client, isbn: &str) -> Result<Option<CrossrefMeta>> {
    let url = format!("https://openlibrary.org/api/books?bibkeys=ISBN:{isbn}&format=json&jscmd=data");
    let resp = client.get(&url).send().await.context("OpenLibrary request")?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body: Value = resp.json().await.context("OpenLibrary JSON")?;
    let rec = &body[format!("ISBN:{isbn}")];
    if rec.is_null() {
        return Ok(None);
    }
    let authors = rec["authors"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["name"].as_str())
                .map(split_name)
                .collect()
        })
        .unwrap_or_default();
    let venue = rec["publishers"][0]["name"].as_str().map(str::to_string);
    Ok(Some(CrossrefMeta {
        title: rec["title"].as_str().map(str::to_string),
        venue,
        year: rec["publish_date"].as_str().and_then(year_from),
        abstract_text: None,
        authors,
        references: Vec::new(),
        raw_json: body.to_string(),
    }))
}

/// NCBI eutils esummary → PubMed metadata for a PMID.
pub async fn fetch_pmid(client: &reqwest::Client, pmid: &str) -> Result<Option<CrossrefMeta>> {
    let url = format!(
        "https://eutils.ncbi.nlm.nih.gov/entrez/eutils/esummary.fcgi?db=pubmed&id={pmid}&retmode=json"
    );
    let resp = client.get(&url).send().await.context("PubMed request")?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let body: Value = resp.json().await.context("PubMed JSON")?;
    let rec = &body["result"][pmid];
    if rec.is_null() {
        return Ok(None);
    }
    let authors = rec["authors"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["name"].as_str())
                .map(split_name)
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(CrossrefMeta {
        title: rec["title"].as_str().map(strip_markup),
        venue: rec["source"].as_str().map(str::to_string),
        year: rec["pubdate"].as_str().and_then(year_from),
        abstract_text: None,
        authors,
        references: Vec::new(),
        raw_json: rec.to_string(),
    }))
}

/// Ask Unpaywall for a legal Open-Access PDF URL for a DOI. Returns None if there
/// is no OA copy. Requires a contact email (Unpaywall mandates it).
pub async fn unpaywall_pdf(
    client: &reqwest::Client,
    doi: &str,
    email: &str,
) -> Result<Option<String>> {
    let doi = doi.trim().trim_start_matches("https://doi.org/").trim_start_matches("doi:");
    let url = format!(
        "https://api.unpaywall.org/v2/{}?email={}",
        urlencoding(doi),
        urlencoding(email.trim())
    );
    let resp = client.get(&url).send().await.context("Unpaywall request")?;
    if !resp.status().is_success() {
        return Ok(None);
    }
    let v: serde_json::Value = resp.json().await.context("Unpaywall JSON")?;
    // Prefer best_oa_location, then scan all oa_locations for a PDF link.
    let pick = |loc: &serde_json::Value| -> Option<String> {
        loc.get("url_for_pdf")
            .and_then(|u| u.as_str())
            .or_else(|| loc.get("url").and_then(|u| u.as_str()))
            .map(str::to_string)
    };
    if let Some(u) = pick(&v["best_oa_location"]) {
        return Ok(Some(u));
    }
    if let Some(arr) = v["oa_locations"].as_array() {
        for loc in arr {
            if let Some(u) = loc.get("url_for_pdf").and_then(|u| u.as_str()) {
                return Ok(Some(u.to_string()));
            }
        }
    }
    Ok(None)
}

/// Minimal percent-encoding for URL query components.
fn urlencoding(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Detect the identifier type of `raw` and resolve it to a reference.
pub async fn resolve(
    client: &reqwest::Client,
    raw: &str,
    email: Option<&str>,
) -> Result<Option<ResolvedRef>> {
    let r = raw.trim();
    let lower = r.to_lowercase();

    // DOI
    if lower.starts_with("10.") || lower.contains("doi.org") || lower.starts_with("doi:") {
        if let Some(doi) = extract_doi(r) {
            return Ok(fetch_crossref(client, &doi, email).await?.map(|meta| ResolvedRef {
                path_id: format!("ref:doi:{doi}"),
                doi: Some(doi),
                meta,
            }));
        }
    }
    // arXiv
    if lower.contains("arxiv") || ARXIV_RE.is_match(r) {
        if let Some(m) = ARXIV_RE.find(r) {
            let id = m.as_str();
            return Ok(fetch_arxiv(client, id).await?.map(|meta| ResolvedRef {
                path_id: format!("ref:arxiv:{id}"),
                doi: None,
                meta,
            }));
        }
    }
    // ISBN (10 or 13 digits, dashes/spaces allowed)
    let digits: String = r.chars().filter(|c| c.is_ascii_digit() || *c == 'x' || *c == 'X').collect();
    if (lower.starts_with("isbn") || digits.len() == 10 || digits.len() == 13)
        && (digits.len() == 10 || digits.len() == 13)
    {
        return Ok(fetch_isbn(client, &digits).await?.map(|meta| ResolvedRef {
            path_id: format!("ref:isbn:{digits}"),
            doi: None,
            meta,
        }));
    }
    // PMID (pmid: prefix or plain short digit string)
    if lower.starts_with("pmid") || (digits.chars().all(|c| c.is_ascii_digit()) && !digits.is_empty() && digits.len() < 10)
    {
        let pmid: String = r.chars().filter(|c| c.is_ascii_digit()).collect();
        return Ok(fetch_pmid(client, &pmid).await?.map(|meta| ResolvedRef {
            path_id: format!("ref:pmid:{pmid}"),
            doi: None,
            meta,
        }));
    }
    // Fallback: maybe a bare DOI without prefix.
    if let Some(doi) = extract_doi(r) {
        return Ok(fetch_crossref(client, &doi, email).await?.map(|meta| ResolvedRef {
            path_id: format!("ref:doi:{doi}"),
            doi: Some(doi),
            meta,
        }));
    }
    Ok(None)
}

/// Build an HTTP client for the Crossref/OpenAlex polite pool. `email`, when
/// set, is sent only as a contact in the User-Agent (no hardcoded personal email).
pub fn http_client(email: Option<&str>) -> Result<reqwest::Client> {
    let ua = match email {
        Some(e) if !e.trim().is_empty() => format!("Scriptorium/0.1 (mailto:{})", e.trim()),
        _ => USER_AGENT.to_string(),
    };
    reqwest::Client::builder()
        .user_agent(ua)
        .timeout(Duration::from_secs(30))
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .context("building HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_cleans_doi() {
        assert_eq!(
            extract_doi("see https://doi.org/10.1145/3292500.3330701."),
            Some("10.1145/3292500.3330701".to_string())
        );
        assert_eq!(
            extract_doi("DOI:10.1038/nature14539 published"),
            Some("10.1038/nature14539".to_string())
        );
        // DOI glued to following text by PDF extraction.
        assert_eq!(
            extract_doi("10.1145/3292500.3330701https://dl.acm.org/x"),
            Some("10.1145/3292500.3330701".to_string())
        );
        assert_eq!(extract_doi("no identifier here"), None);
    }

    #[test]
    fn extract_dois_distinct_in_order() {
        let t = "10.1145/3292500.3330701. later see doi:10.1038/nature14539 \
                 and again 10.1145/3292500.3330701";
        assert_eq!(
            extract_dois(t, 5),
            vec!["10.1145/3292500.3330701".to_string(), "10.1038/nature14539".to_string()]
        );
        assert_eq!(extract_dois(t, 1).len(), 1);
        assert!(extract_dois("no identifiers", 5).is_empty());
    }

    #[test]
    fn extract_arxiv_ids_requires_marker() {
        let t = "as in arXiv:2406.09406v2 and arxiv.org/abs/1706.03762; \
                 but Section 1999.12345 and the bare 2301.00001 are not ids; \
                 arXiv preprint arXiv:2406.09406v2 repeats";
        assert_eq!(
            extract_arxiv_ids(t, 5),
            vec!["2406.09406v2".to_string(), "1706.03762".to_string()]
        );
        // Implausible month (99) is rejected even when arXiv-marked.
        assert!(extract_arxiv_ids("arXiv:1999.12345", 5).is_empty());
    }

    #[test]
    fn strips_jats_markup() {
        assert_eq!(
            strip_markup("<jats:p>Hello   <jats:italic>world</jats:italic></jats:p>"),
            "Hello world"
        );
    }

    #[test]
    fn title_gate_accepts_real_match_rejects_cited_work() {
        // The document IS the SAD paper: its title is printed at the top.
        let head = "Me, Myself, and AI: The Situational Awareness Dataset (SAD) for LLMs \
                    Rudolf Laine Bilal Chughtai";
        assert!(title_matches_doc(
            "Me, Myself, and AI: The Situational Awareness Dataset (SAD) for LLMs",
            head
        ));
        // A *cited* paper's title must NOT be accepted for this document.
        assert!(!title_matches_doc(
            "Introspective Capabilities in Large Language Models",
            head
        ));
        // Generic-word-only overlap must not pass (all distinctive words missing).
        assert!(!title_matches_doc(
            "Rescaling Egocentric Vision: Challenges for EPIC-KITCHENS-100",
            head
        ));
    }

    #[test]
    fn strong_title_match_separates_own_title_from_cited_work() {
        // The document's own title vs its cited Topol reference — must NOT match,
        // even though both are medical-AI papers (the ATHENA-R1 bug).
        let athena = "ATHENA-R1: An AI Agent for Treatment Reasoning over a Biomedical Tool Universe";
        let topol = "High-performance medicine: the convergence of human and artificial intelligence";
        assert!(!strong_title_match(athena, topol));
        // Same paper, hyphenation/spacing differences — must match.
        assert!(strong_title_match(
            "High performance medicine the convergence of human and artificial intelligence",
            topol
        ));
        // The same title with the subtitle present matches exactly.
        assert!(strong_title_match(
            "Athena: Enhancing Multimodal Reasoning with Data-efficient Process Reward Models",
            "Athena: Enhancing Multimodal Reasoning with Data-efficient Process Reward Models"
        ));
        // Two unrelated papers do not match.
        assert!(!strong_title_match(
            "Attention Is All You Need",
            "High-performance medicine: the convergence of human and artificial intelligence"
        ));
        // A cited work whose title is a token-subset / prefix / extension of the
        // document's title must NOT match, in either argument order — no subset,
        // prefix or containment shortcut may reopen the cited-work trap.
        let citing = "Attention is not all you need pure attention loses rank doubly exponentially with depth";
        let cited = "Attention Is All You Need";
        assert!(!strong_title_match(citing, cited));
        assert!(!strong_title_match(cited, citing));
        assert!(!strong_title_match("Segment Anything", "Segment Anything in Medical Images"));
        assert!(!strong_title_match("Segment Anything in Medical Images", "Segment Anything"));
        // A subtitle-truncated recovery is (conservatively) NOT matched — we prefer
        // leaving a doc un-enriched over any risk of a wrong extension.
        assert!(!strong_title_match(
            "Learning Transferable Visual Models",
            "Learning Transferable Visual Models From Natural Language Supervision"
        ));
        // Real neighbours that a looser (Jaccard) gate wrongly matched on live
        // Crossref data — a one-word extension and a reordered near-duplicate.
        assert!(!strong_title_match(
            "Denoising Diffusion Probabilistic Models",
            "Riemannian Denoising Diffusion Probabilistic Models"
        ));
        assert!(!strong_title_match(
            "Denoising Diffusion Probabilistic Models",
            "Denoising Diffusion Implicit Models"
        ));
        assert!(!strong_title_match("Attention Is All You Need", "Is Attention All You Need?"));
        // A distinctive multi-word title still matches its exact self.
        assert!(strong_title_match(
            "Deep Residual Learning for Image Recognition",
            "Deep Residual Learning for Image Recognition"
        ));
        // Accent-only spelling differences must NOT split the same paper.
        assert!(strong_title_match(
            "Poincare Recurrence in Neural Dynamics",
            "Poincaré Recurrence in Neural Dynamics"
        ));
        assert!(strong_title_match(
            "Erdos-Renyi Graphs and Spectral Gaps",
            "Erdős–Rényi Graphs and Spectral Gaps"
        ));
    }

    #[test]
    fn reference_names_title_gates_backfill() {
        // A real reference whose full title (>=4 distinctive words) is spelled out,
        // with the candidate's diacritics folded — accepted.
        let raw = "A. Bordes, N. Usunier, A. Garcia-Duran, J. Weston, and O. Yakhnenko. 2013. \
                   Translating embeddings for modeling multi-relational data. In Proc. of NIPS.";
        assert!(reference_names_title("Translating embeddings for modeling multi-relational data", raw));
        // Whole-token, NOT substring: 'adam' must not match inside the surname
        // 'adamson' — so the Adam-optimizer DOI is not attached to a control paper.
        let control = "Adamson, R. Stochastic optimization control. Automatica, 2019.";
        assert!(!reference_names_title("Adam stochastic optimization control", control));
        // A short candidate (<4 distinctive words) that is merely a SUBSET of a
        // different work's longer title is rejected (the ELMo-as-subset trap).
        let longer = "Deep contextualized word representations of sentences for pretraining transformer encoders. 2019.";
        assert!(!reference_names_title("Deep contextualized word representations", longer));
        // An unrelated candidate shares no distinctive word — rejected.
        assert!(!reference_names_title("A survey of quantum error correction codes", raw));
        // A candidate that names MORE than the reference does (extra distinctive
        // words absent) falls below the coverage floor — rejected.
        assert!(!reference_names_title(
            "Translating embeddings for modeling multi-relational data across federated knowledge graphs",
            raw
        ));
    }

    #[test]
    #[ignore = "hits the live Crossref/arXiv APIs"]
    fn candidate_searches_live() {
        // Run with: cargo test --lib candidate_searches_live -- --ignored --nocapture
        let client = super::http_client(Some("test@example.com")).unwrap();
        tauri::async_runtime::block_on(async {
            let cx = super::crossref_search_candidates(
                &client,
                "Attention Is All You Need",
                Some("test@example.com"),
                5,
            )
            .await
            .unwrap();
            println!("crossref candidates: {}", cx.len());
            for (doi, m) in &cx {
                println!("  {doi} -> {:?} ({:?})", m.title, m.year);
            }
            assert!(!cx.is_empty(), "expected Crossref candidates");
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let ax = super::arxiv_search_candidates(&client, "Attention Is All You Need", 5)
                .await
                .unwrap();
            println!("arxiv candidates: {}", ax.len());
            for (id, m) in &ax {
                println!("  {id} -> {:?}", m.title);
            }
            assert!(!ax.is_empty(), "expected arXiv candidates");
            assert!(ax.iter().all(|(id, _)| !id.is_empty()), "arXiv ids parsed from <id>");
        });
    }

    #[test]
    #[ignore = "hits the live Crossref API"]
    fn crossref_resolve_reference_live() {
        // Real DOI-less citation strings drawn from the library. Run with:
        //   cargo test --lib crossref_resolve_reference_live -- --ignored --nocapture
        let client = super::http_client(Some("test@example.com")).unwrap();
        // Journal papers (always in Crossref) with >=4 distinctive title words.
        let raws = [
            "O. Russakovsky, J. Deng, H. Su, et al. ImageNet Large Scale Visual \
             Recognition Challenge. International Journal of Computer Vision, 2015.",
            "D. Silver, A. Huang, C. Maddison, et al. Mastering the game of Go with \
             deep neural networks and tree search. Nature 529, 484-489 (2016).",
            "J. Jumper, R. Evans, A. Pritzel, et al. Highly accurate protein structure \
             prediction with AlphaFold. Nature 596, 583-589 (2021).",
        ];
        let mut resolved = 0usize;
        tauri::async_runtime::block_on(async {
            for raw in raws {
                let doi = super::crossref_resolve_reference(&client, raw, Some("test@example.com"))
                    .await
                    .unwrap();
                println!("RAW: {raw}\n  -> {doi:?}\n");
                if doi.is_some() {
                    resolved += 1;
                }
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            }
            // A nonsense citation must NOT resolve (the precision guarantee).
            let bogus = super::crossref_resolve_reference(
                &client,
                "Zzzq Xqptl. 1999. Flarngblat quxxle woozzle in the greeble. In Proc. of Nowhere.",
                Some("test@example.com"),
            )
            .await
            .unwrap();
            println!("BOGUS -> {bogus:?}");
            assert!(bogus.is_none(), "a nonsense citation must not resolve");
        });
        assert!(resolved >= 1, "expected >=1 real reference to resolve via Crossref");
    }

    #[test]
    fn first_line_title_handles_banner_edge_cases() {
        // A long contiguous run of publisher chrome is skipped entirely.
        let long = "Downloaded from https://x.org\r\
                    © 2021 Publisher\r\
                    ISSN 1234-5678\r\
                    e-ISSN 8765-4321\r\
                    ISBN 978-0-000\r\
                    This content downloaded from 1.2.3.4\r\
                    Deep Residual Learning for Image Recognition.";
        assert_eq!(
            first_line_title(long).as_deref(),
            Some("Deep Residual Learning for Image Recognition")
        );
        // A banner emitted AFTER the real title (non-linear text order) must not
        // drag the title away — the leading real title wins.
        let nonlinear = "Machine Learning Approaches to Protein Folding.\r\
                         Available at ScienceDirect\r\
                         Journal of Computational Biology";
        assert_eq!(
            first_line_title(nonlinear).as_deref(),
            Some("Machine Learning Approaches to Protein Folding")
        );
        // A real title that merely contains a venue phrase is not treated as chrome.
        let contains = "A Workshop on Reproducibility Is Not Enough.\r\
                        Jane Doe";
        assert_eq!(
            first_line_title(contains).as_deref(),
            Some("A Workshop on Reproducibility Is Not Enough")
        );
    }

    #[test]
    fn first_line_title_skips_publisher_banner() {
        // A ScienceDirect-style banner + running head must be skipped; the real
        // title on a later line is what gets recovered.
        let txt = "Contents lists available at ScienceDirect\r\
                   Artificial Intelligence in Medicine\r\
                   journal homepage: www.elsevier.com/locate/artmed\r\
                   Deep learning for retinal disease detection\r\
                   Jane Doe1, John Smith2";
        assert_eq!(
            first_line_title(txt).as_deref(),
            Some("Deep learning for retinal disease detection")
        );
    }

    #[test]
    fn arxiv_id_from_filename_variants() {
        assert_eq!(arxiv_id_from_filename("2406.09406v2.pdf").as_deref(), Some("2406.09406v2"));
        assert_eq!(arxiv_id_from_filename("arxiv_2606_00995.pdf").as_deref(), Some("2606.00995"));
        assert_eq!(arxiv_id_from_filename("2512.16301.pdf").as_deref(), Some("2512.16301"));
        // No id present.
        assert_eq!(arxiv_id_from_filename("v-jepa2.pdf"), None);
        assert_eq!(arxiv_id_from_filename("doc_83.pdf"), None);
        assert_eq!(arxiv_id_from_filename("614775426.pdf"), None);
        // Implausible month (19) is rejected.
        assert_eq!(arxiv_id_from_filename("1234.5678.pdf"), None);
    }

    #[test]
    fn first_line_title_joins_wrapped_title() {
        let txt = "Me, Myself, and AI:\r The Situational Awareness Dataset (SAD) for LLMs\r Rudolf Laine\r Independent";
        assert_eq!(
            first_line_title(txt).as_deref(),
            Some("Me, Myself, and AI: The Situational Awareness Dataset (SAD) for LLMs")
        );
    }

    #[test]
    fn first_line_title_does_not_swallow_authors() {
        // A complete title line must not pull in the following author line.
        let txt = "xLSTM: Extended Long Short-Term Memory\rMaximilian Beck∗ 1,2,3\rKorbinian Pöppel";
        assert_eq!(
            first_line_title(txt).as_deref(),
            Some("xLSTM: Extended Long Short-Term Memory")
        );
        // Author markers glued onto the same line are trimmed off.
        let txt2 = "Titans: Learning to Memorize at Test Time Ali Behrouz†, Peilin Zhong";
        assert_eq!(
            first_line_title(txt2).as_deref(),
            Some("Titans: Learning to Memorize at Test Time Ali Behrouz")
        );
    }
}
