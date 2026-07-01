//! Metadata enrichment: find a document's DOI, look it up on Crossref, and
//! write the bibliographic data (title, authors, year, venue, abstract,
//! references) back into the database.

use anyhow::{bail, Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
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

/// Distinctive (≥4-letter, non-stopword) lowercase words of a title, de-duplicated.
fn sig_title_words(title: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for w in title.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
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
    let head_l = head.to_lowercase();
    let words = sig_title_words(title);
    if words.is_empty() {
        // No distinctive words (very short/generic title): require the trimmed
        // title itself to appear near the top.
        let t = title.trim().to_lowercase();
        return t.len() >= 4 && head_l.contains(&t);
    }
    let hits = words.iter().filter(|w| head_l.contains(w.as_str())).count();
    hits * 2 >= words.len() // ≥ 50% of distinctive title words present
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

/// Best-effort title from the start of the extracted PDF text: the first
/// non-empty line, plus the next one when the first looks like a wrapped title
/// (short, or ends with a colon). Used to recover a sensible title for a
/// mis-enriched document that has no arXiv id in its filename.
pub fn first_line_title(fulltext: &str) -> Option<String> {
    let mut lines = fulltext
        .split(|c: char| c == '\n' || c == '\r')
        .map(str::trim)
        .filter(|l| !l.is_empty());
    let first = lines.next()?;
    let mut title = first.to_string();
    // Append the next line when the first looks like a wrapped title (does not end
    // with sentence punctuation) and the next line is a title continuation — NOT
    // the author/affiliation line, which carries superscript markers or digits.
    let l1_complete = first.ends_with('.') || first.ends_with('?') || first.ends_with('!');
    if !l1_complete {
        if let Some(second) = lines.next() {
            let low = second.to_lowercase();
            let looks_meta = low.starts_with("abstract")
                || low.starts_with("introduction")
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
        .trim_end_matches(|c: char| matches!(c, ':' | ' ' | ',' | '-'))
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
    let msg = &body["message"];

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

    let year = msg["issued"]["date-parts"][0][0].as_i64();

    Ok(Some(CrossrefMeta {
        title: first_str(&msg["title"]),
        venue: first_str(&msg["container-title"]),
        year,
        abstract_text: msg["abstract"].as_str().map(strip_markup),
        authors,
        references,
        raw_json: body.to_string(),
    }))
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

    // Avoid violating UNIQUE(doi): if another document already owns this DOI,
    // apply the rest of the metadata but leave this row's doi unchanged.
    let dup: Option<i64> = tx
        .query_row(
            "SELECT id FROM documents WHERE doi = ?1 AND id <> ?2",
            params![doi, id],
            |r| r.get(0),
        )
        .optional()?;
    let doi_to_set: Option<&str> = if dup.is_some() { None } else { Some(doi) };

    tx.execute(
        "UPDATE documents
         SET title = COALESCE(?1, title), year = ?2, venue = ?3,
             doi = COALESCE(?4, doi), abstract = ?5
         WHERE id = ?6",
        params![meta.title, meta.year, meta.venue, doi_to_set, meta.abstract_text, id],
    )
    .context("updating document with metadata")?;

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
    let entry = &body[estart..];
    let title = between(entry, "<title>", "</title>");
    let mut authors = Vec::new();
    let mut rest = entry;
    while let Some(name) = between(rest, "<name>", "</name>") {
        authors.push(split_name(&name));
        let cut = rest.find("</name>").map(|i| i + 7).unwrap_or(rest.len());
        rest = &rest[cut..];
    }
    let year = between(entry, "<published>", "</published>").and_then(|p| year_from(&p));
    Ok(Some(CrossrefMeta {
        title,
        venue: Some("arXiv".to_string()),
        year,
        abstract_text: between(entry, "<summary>", "</summary>"),
        authors,
        references: Vec::new(),
        raw_json: String::new(),
    }))
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
