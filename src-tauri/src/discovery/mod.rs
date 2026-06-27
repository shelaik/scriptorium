//! Online paper discovery: search arXiv and OpenAlex, returning normalized
//! results with a legal Open-Access PDF URL when available. Network-only;
//! gated behind an opt-in setting at the command layer.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A normalized search hit from any source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub source: String, // "arxiv" | "openalex" | "ads"
    pub external_id: String,
    pub doi: Option<String>,
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    pub abstract_text: Option<String>,
    /// Legal Open-Access PDF URL, if one exists.
    pub oa_pdf_url: Option<String>,
    /// Landing page (DOI / abstract page) to open in the browser.
    pub url: Option<String>,
    pub is_oa: bool,
    pub citations: i64,
    /// Set by the command layer: already present in the local library.
    pub in_library: bool,
    /// Set by the command layer: a GitHub repo URL detected in title/abstract.
    #[serde(default)]
    pub github_url: Option<String>,
    /// Set by the command layer: "published" | "preprint" | "preprint_reviewed".
    #[serde(default)]
    pub pub_status: Option<String>,
}

/// Search filters shared by sources.
#[derive(Debug, Clone, Deserialize)]
pub struct Filters {
    pub year_from: Option<i64>,
    pub year_to: Option<i64>,
    pub oa_only: bool,
    pub sort: String, // "relevance" | "date" | "citations"
    /// Optional author name to narrow by (separate from the free-text query).
    pub author: Option<String>,
}

/// Trimmed non-empty form of an optional string filter.
fn nonempty(s: &Option<String>) -> Option<&str> {
    s.as_deref().map(str::trim).filter(|s| !s.is_empty())
}

/// DOI prefixes that identify a preprint server (not a peer-reviewed publication).
const PREPRINT_DOI_PREFIXES: &[&str] = &["10.48550/arxiv", "10.1101", "10.21203", "10.26434"];

/// Classify publication status from metadata:
/// `"published"` (peer-reviewed), `"preprint"` (preprint only), or
/// `"preprint_reviewed"` (a preprint whose peer-reviewed version exists, via a
/// non-preprint DOI). `hint` is the source name or the stored path.
pub fn classify_pub_status(doi: Option<&str>, venue: Option<&str>, hint: Option<&str>) -> Option<String> {
    let dl = doi.map(|d| d.to_lowercase());
    let vl = venue.unwrap_or("").to_lowercase();
    let hl = hint.unwrap_or("").to_lowercase();
    let is_preprint_doi = dl
        .as_deref()
        .map(|d| PREPRINT_DOI_PREFIXES.iter().any(|p| d.starts_with(p)))
        .unwrap_or(false);
    const PREPRINT_KW: &[&str] = &[
        "arxiv", "biorxiv", "medrxiv", "chemrxiv", "preprint", "ssrn", "research square",
        "researchsquare", "osf", "techrxiv", "authorea",
    ];
    let preprintish = is_preprint_doi || PREPRINT_KW.iter().any(|k| vl.contains(k) || hl.contains(k));
    let has_published_doi = dl.is_some() && !is_preprint_doi;
    if preprintish {
        Some(if has_published_doi { "preprint_reviewed" } else { "preprint" }.to_string())
    } else if has_published_doi || !vl.is_empty() {
        Some("published".to_string())
    } else {
        None
    }
}

fn enc(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn xml_between(s: &str, a: &str, b: &str) -> Option<String> {
    let start = s.find(a)? + a.len();
    let end = s[start..].find(b)? + start;
    let raw = &s[start..end];
    let cleaned = raw
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"");
    Some(cleaned.split_whitespace().collect::<Vec<_>>().join(" "))
}

fn year_of(s: &str) -> Option<i64> {
    s.get(0..4).and_then(|y| y.parse::<i64>().ok())
}

/// Search arXiv (Atom feed). arXiv is entirely Open Access.
pub async fn search_arxiv(
    client: &reqwest::Client,
    query: &str,
    filters: &Filters,
) -> Result<Vec<SearchResult>> {
    let sort_by = match filters.sort.as_str() {
        "date" => "submittedDate",
        _ => "relevance",
    };
    // Combine the free-text query with an optional author term (arXiv `au:`).
    // Values are percent-encoded; prefixes/operators stay literal.
    let mut terms: Vec<String> = Vec::new();
    if !query.trim().is_empty() {
        terms.push(format!("all:{}", enc(query.trim())));
    }
    if let Some(a) = nonempty(&filters.author) {
        terms.push(format!("au:{}", enc(a)));
    }
    if terms.is_empty() {
        return Ok(Vec::new());
    }
    let search_query = terms.join("+AND+");
    let url = format!(
        "https://export.arxiv.org/api/query?search_query={}&start=0&max_results=40&sortBy={}&sortOrder=descending",
        search_query,
        sort_by
    );
    let resp = client.get(&url).send().await.context("arXiv request")?;
    if !resp.status().is_success() {
        anyhow::bail!("arXiv HTTP {}", resp.status());
    }
    let body = resp.text().await.context("arXiv body")?;

    let mut results = Vec::new();
    for entry in body.split("<entry>").skip(1) {
        let id_url = xml_between(entry, "<id>", "</id>").unwrap_or_default();
        // id like http://arxiv.org/abs/2301.12345v1 -> 2301.12345
        let aid = id_url
            .rsplit('/')
            .next()
            .unwrap_or("")
            .split('v')
            .next()
            .unwrap_or("")
            .to_string();
        if aid.is_empty() {
            continue;
        }
        let title = xml_between(entry, "<title>", "</title>");
        let mut authors = Vec::new();
        let mut rest = entry;
        while let Some(name) = xml_between(rest, "<name>", "</name>") {
            authors.push(name);
            let cut = rest.find("</name>").map(|i| i + 7).unwrap_or(rest.len());
            rest = &rest[cut..];
        }
        let year = xml_between(entry, "<published>", "</published>").and_then(|p| year_of(&p));
        // Apply client-side year filtering (arXiv date filters are awkward).
        if let Some(y) = year {
            if filters.year_from.is_some_and(|f| y < f) || filters.year_to.is_some_and(|t| y > t) {
                continue;
            }
        }
        let doi = xml_between(entry, "<arxiv:doi>", "</arxiv:doi>");
        results.push(SearchResult {
            source: "arxiv".to_string(),
            external_id: aid.clone(),
            doi,
            title,
            authors,
            year,
            venue: Some("arXiv".to_string()),
            abstract_text: xml_between(entry, "<summary>", "</summary>"),
            oa_pdf_url: Some(format!("https://arxiv.org/pdf/{aid}")),
            url: Some(format!("https://arxiv.org/abs/{aid}")),
            is_oa: true,
            citations: 0,
            in_library: false,
            github_url: None,
            pub_status: None,
        });
    }
    Ok(results)
}

/// Reconstruct an abstract from OpenAlex's `abstract_inverted_index`
/// (`{word: [positions...]}`) by placing each word at every position it occupies.
fn decode_inverted_abstract(v: &Value) -> Option<String> {
    let map = v.as_object()?;
    if map.is_empty() {
        return None;
    }
    let mut slots: Vec<(usize, &str)> = Vec::new();
    for (word, positions) in map {
        if let Some(arr) = positions.as_array() {
            for p in arr {
                if let Some(i) = p.as_u64() {
                    slots.push((i as usize, word.as_str()));
                }
            }
        }
    }
    if slots.is_empty() {
        return None;
    }
    slots.sort_by_key(|(i, _)| *i);
    let text = slots.into_iter().map(|(_, w)| w).collect::<Vec<_>>().join(" ");
    Some(text)
}

fn clean_doi(s: &str) -> Option<String> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    Some(s.trim_start_matches("https://doi.org/").trim_start_matches("http://doi.org/").to_lowercase())
}

/// Search OpenAlex works. Returns OA PDF URLs from best_oa_location.
pub async fn search_openalex(
    client: &reqwest::Client,
    query: &str,
    filters: &Filters,
    api_key: &str,
) -> Result<Vec<SearchResult>> {
    let mut filter_parts: Vec<String> = Vec::new();
    if let Some(f) = filters.year_from {
        filter_parts.push(format!("from_publication_date:{f}-01-01"));
    }
    if let Some(t) = filters.year_to {
        filter_parts.push(format!("to_publication_date:{t}-12-31"));
    }
    if filters.oa_only {
        filter_parts.push("is_oa:true".to_string());
    }
    if let Some(a) = nonempty(&filters.author) {
        // OpenAlex filter values use `,` as AND and `|` as OR; strip them so a
        // name can't inject extra clauses.
        let safe = a.replace([',', '|'], " ");
        filter_parts.push(format!("raw_author_name.search:{}", safe));
    }
    let sort = match filters.sort.as_str() {
        "date" => "&sort=publication_date:desc",
        "citations" => "&sort=cited_by_count:desc",
        _ => "",
    };
    let filter = if filter_parts.is_empty() {
        String::new()
    } else {
        format!("&filter={}", filter_parts.join(","))
    };
    let select = "id,doi,title,publication_year,authorships,primary_location,open_access,best_oa_location,cited_by_count,abstract_inverted_index";
    // OpenAlex's free public API works WITHOUT a key (polite pool via mailled
    // User-Agent). Only send api_key when one is actually configured — an empty
    // `api_key=` is treated as an invalid premium credential and rejected (403).
    let key_part = if api_key.trim().is_empty() {
        String::new()
    } else {
        format!("&api_key={}", enc(api_key.trim()))
    };
    let url = format!(
        "https://api.openalex.org/works?search={}&per-page=40{}{}&select={}{}",
        enc(query),
        filter,
        sort,
        select,
        key_part
    );
    let resp = client.get(&url).send().await.context("OpenAlex request")?;
    let status = resp.status();
    if status.as_u16() == 403 || status.as_u16() == 401 || status.as_u16() == 409 {
        anyhow::bail!("OpenAlex richiede una chiave API valida (HTTP {status})");
    }
    if !status.is_success() {
        anyhow::bail!("OpenAlex HTTP {status}");
    }
    let body: Value = resp.json().await.context("OpenAlex JSON")?;
    let results = body["results"]
        .as_array()
        .map(|arr| arr.iter().map(openalex_to_result).collect())
        .unwrap_or_default();
    Ok(results)
}

/// Map one OpenAlex `work` JSON object into a normalized [`SearchResult`].
/// Shared by full-text search and the citation-neighbours explorer.
fn openalex_to_result(w: &Value) -> SearchResult {
    let authors = w["authorships"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x["author"]["display_name"].as_str())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let oa_pdf_url = w["best_oa_location"]["pdf_url"]
        .as_str()
        .or_else(|| w["open_access"]["oa_url"].as_str())
        .map(str::to_string);
    let full_id = w["id"].as_str().unwrap_or("");
    let id = full_id.rsplit('/').next().unwrap_or("").to_string();
    let doi = w["doi"].as_str().and_then(clean_doi);
    // Prefer the DOI landing page; fall back to the OpenAlex work page.
    let url = doi
        .as_ref()
        .map(|d| format!("https://doi.org/{d}"))
        .or_else(|| (!full_id.is_empty()).then(|| full_id.to_string()));
    SearchResult {
        source: "openalex".to_string(),
        external_id: id,
        doi,
        title: w["title"].as_str().map(str::to_string),
        authors,
        year: w["publication_year"].as_i64(),
        venue: w["primary_location"]["source"]["display_name"].as_str().map(str::to_string),
        abstract_text: decode_inverted_abstract(&w["abstract_inverted_index"]),
        url,
        is_oa: w["open_access"]["is_oa"].as_bool().unwrap_or(false),
        oa_pdf_url,
        citations: w["cited_by_count"].as_i64().unwrap_or(0),
        in_library: false,
        github_url: None,
        pub_status: None,
    }
}

/// References (works this paper cites) and citations (works that cite it),
/// fetched from OpenAlex for the citation-graph explorer.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CitationNeighbors {
    pub references: Vec<SearchResult>,
    pub citations: Vec<SearchResult>,
    /// True when the seed paper could not be resolved on OpenAlex.
    pub seed_unresolved: bool,
}

/// Extract the bare OpenAlex work id (e.g. `W123`) from a full work URL/id.
fn openalex_work_id(full: &str) -> &str {
    full.rsplit('/').next().unwrap_or(full)
}

/// "Snowball" neighbours of a paper identified by `doi`: the works it references
/// (backward) and the works that cite it (forward, ranked by citation count).
/// Both come from OpenAlex. `max` caps each list. Network-gated by the caller.
pub async fn openalex_neighbors(
    client: &reqwest::Client,
    doi: &str,
    api_key: &str,
    max: usize,
) -> Result<CitationNeighbors> {
    let select = "id,doi,title,publication_year,authorships,primary_location,open_access,best_oa_location,cited_by_count,abstract_inverted_index";
    let key_part = if api_key.trim().is_empty() {
        String::new()
    } else {
        format!("&api_key={}", enc(api_key.trim()))
    };
    let max = max.clamp(1, 100);

    // 1. Resolve the seed work by DOI to get its id + referenced_works. Use the
    //    `filter=doi:` query form (not a path lookup): DOIs contain slashes that
    //    break percent-encoded path routing, but encode cleanly as a query value.
    let doi_clean = doi
        .trim()
        .trim_start_matches("https://doi.org/")
        .trim_start_matches("http://doi.org/")
        .trim_start_matches("doi:");
    let seed_url = format!(
        "https://api.openalex.org/works?filter=doi:{}&per-page=1&select=id,referenced_works{}",
        enc(doi_clean),
        key_part
    );
    let resp = client.get(&seed_url).send().await.context("OpenAlex seed request")?;
    if resp.status().as_u16() == 403 || resp.status().as_u16() == 401 {
        anyhow::bail!("OpenAlex richiede una chiave API valida (HTTP {})", resp.status());
    }
    if !resp.status().is_success() {
        anyhow::bail!("OpenAlex HTTP {}", resp.status());
    }
    let body: Value = resp.json().await.context("OpenAlex seed JSON")?;
    let seed = body["results"].get(0).cloned().unwrap_or(Value::Null);
    let work_id = openalex_work_id(seed["id"].as_str().unwrap_or("")).to_string();
    if work_id.is_empty() {
        return Ok(CitationNeighbors { references: Vec::new(), citations: Vec::new(), seed_unresolved: true });
    }
    let referenced: Vec<String> = seed["referenced_works"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).map(|s| openalex_work_id(s).to_string()).collect())
        .unwrap_or_default();

    // 2. Backward — metadata for (up to `max`) referenced works, via an OR filter.
    let mut references = Vec::new();
    let ids: Vec<String> = referenced.into_iter().take(max).collect();
    if !ids.is_empty() {
        let url = format!(
            "https://api.openalex.org/works?filter=openalex:{}&per-page={}&select={}{}",
            ids.join("|"),
            max,
            select,
            key_part
        );
        if let Ok(r) = client.get(&url).send().await {
            if r.status().is_success() {
                if let Ok(body) = r.json::<Value>().await {
                    references = body["results"].as_array().map(|a| a.iter().map(openalex_to_result).collect()).unwrap_or_default();
                }
            }
        }
        references.sort_by(|a, b| b.citations.cmp(&a.citations));
    }

    // 3. Forward — top works that cite the seed, ranked by citation count.
    let cites_url = format!(
        "https://api.openalex.org/works?filter=cites:{}&sort=cited_by_count:desc&per-page={}&select={}{}",
        work_id, max, select, key_part
    );
    let mut citations = Vec::new();
    if let Ok(r) = client.get(&cites_url).send().await {
        if r.status().is_success() {
            if let Ok(body) = r.json::<Value>().await {
                citations = body["results"].as_array().map(|a| a.iter().map(openalex_to_result).collect()).unwrap_or_default();
            }
        }
    }

    Ok(CitationNeighbors { references, citations, seed_unresolved: false })
}

/// First element of a JSON string-array (ADS returns title/doi as arrays).
fn first_str(v: &Value) -> Option<String> {
    v.as_array()
        .and_then(|a| a.iter().find_map(|x| x.as_str()))
        .map(str::to_string)
}

/// Search NASA ADS (Astrophysics Data System). Requires a (free) API token.
/// OA PDFs are exposed only via the arXiv eprint when present.
pub async fn search_ads(
    client: &reqwest::Client,
    query: &str,
    filters: &Filters,
    token: &str,
) -> Result<Vec<SearchResult>> {
    if token.trim().is_empty() {
        anyhow::bail!("ADS richiede un token API (impostalo nelle impostazioni)");
    }
    // Build the ADS query string (implicit AND between clauses).
    let mut q_parts: Vec<String> = Vec::new();
    if !query.trim().is_empty() {
        q_parts.push(query.trim().to_string());
    }
    if let Some(a) = nonempty(&filters.author) {
        q_parts.push(format!("author:\"{}\"", a.replace('"', " ")));
    }
    // ADS only documents the *closed* hyphen range `year:from-to`; bare-sided
    // forms (`year:2020-`) are not valid, so bound the open side explicitly.
    match (filters.year_from, filters.year_to) {
        (Some(f), Some(t)) => q_parts.push(format!("year:{f}-{t}")),
        (Some(f), None) => q_parts.push(format!("year:{f}-9999")),
        (None, Some(t)) => q_parts.push(format!("year:0-{t}")),
        (None, None) => {}
    }
    if filters.oa_only {
        q_parts.push("property:openaccess".to_string());
    }
    if q_parts.is_empty() {
        return Ok(Vec::new());
    }
    let q = q_parts.join(" ");
    let sort = match filters.sort.as_str() {
        "date" => format!("&sort={}", enc("date desc")),
        "citations" => format!("&sort={}", enc("citation_count desc")),
        _ => String::new(),
    };
    let fl = "bibcode,title,author,year,pub,doi,citation_count,abstract,identifier,property";
    let url = format!(
        "https://api.adsabs.harvard.edu/v1/search/query?q={}&fl={}&rows=40{}",
        enc(&q),
        fl,
        sort
    );
    let resp = client
        .get(&url)
        .bearer_auth(token.trim())
        .send()
        .await
        .context("ADS request")?;
    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        anyhow::bail!("ADS richiede un token API valido (HTTP {status})");
    }
    if !status.is_success() {
        anyhow::bail!("ADS HTTP {status}");
    }
    let body: Value = resp.json().await.context("ADS JSON")?;
    let mut results = Vec::new();
    for d in body["response"]["docs"].as_array().cloned().unwrap_or_default() {
        let bibcode = match d["bibcode"].as_str() {
            Some(b) if !b.is_empty() => b.to_string(),
            _ => continue,
        };
        let authors = d["author"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str()).map(str::to_string).collect())
            .unwrap_or_default();
        let year = d["year"]
            .as_i64()
            .or_else(|| d["year"].as_str().and_then(|s| s.trim().parse().ok()));
        // The arXiv eprint, when present, is the reliably-fetchable OA PDF.
        let arxiv_id = d["identifier"].as_array().and_then(|a| {
            a.iter().filter_map(|x| x.as_str()).find_map(|s| {
                s.to_ascii_lowercase().strip_prefix("arxiv:").map(|_| s[6..].to_string())
            })
        });
        let oa_pdf_url = arxiv_id.as_ref().map(|id| format!("https://arxiv.org/pdf/{id}"));
        let property_oa = d["property"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .any(|p| p.to_ascii_uppercase().contains("OPENACCESS"))
            })
            .unwrap_or(false);
        results.push(SearchResult {
            source: "ads".to_string(),
            external_id: bibcode.clone(),
            doi: first_str(&d["doi"]).as_deref().and_then(clean_doi),
            title: first_str(&d["title"]),
            authors,
            year,
            venue: d["pub"].as_str().map(str::to_string),
            abstract_text: d["abstract"].as_str().map(str::to_string),
            url: Some(format!("https://ui.adsabs.harvard.edu/abs/{bibcode}")),
            is_oa: property_oa || oa_pdf_url.is_some(),
            oa_pdf_url,
            citations: d["citation_count"].as_i64().unwrap_or(0),
            in_library: false,
            github_url: None,
            pub_status: None,
        });
    }
    Ok(results)
}

/// Search Semantic Scholar (Allen AI). Free; an optional API key raises rate limits.
pub async fn search_semantic_scholar(
    client: &reqwest::Client,
    query: &str,
    filters: &Filters,
    api_key: &str,
) -> Result<Vec<SearchResult>> {
    let mut q = query.trim().to_string();
    if let Some(a) = nonempty(&filters.author) {
        q = format!("{q} {a}");
    }
    if q.trim().is_empty() {
        return Ok(Vec::new());
    }
    let fields = "title,abstract,year,authors,venue,externalIds,openAccessPdf,citationCount,isOpenAccess";
    let mut url = format!(
        "https://api.semanticscholar.org/graph/v1/paper/search?query={}&limit=40&fields={}",
        enc(q.trim()),
        fields
    );
    match (filters.year_from, filters.year_to) {
        (Some(f), Some(t)) => url.push_str(&format!("&year={f}-{t}")),
        (Some(f), None) => url.push_str(&format!("&year={f}-")),
        (None, Some(t)) => url.push_str(&format!("&year=-{t}")),
        (None, None) => {}
    }
    if filters.oa_only {
        url.push_str("&openAccessPdf");
    }
    let mut req = client.get(&url);
    if !api_key.trim().is_empty() {
        req = req.header("x-api-key", api_key.trim());
    }
    let resp = req.send().await.context("Semantic Scholar request")?;
    let status = resp.status();
    if status.as_u16() == 429 {
        anyhow::bail!("Semantic Scholar: troppe richieste, riprova tra poco (o imposta una API key)");
    }
    if !status.is_success() {
        anyhow::bail!("Semantic Scholar HTTP {status}");
    }
    let body: Value = resp.json().await.context("Semantic Scholar JSON")?;
    let mut results = Vec::new();
    for w in body["data"].as_array().cloned().unwrap_or_default() {
        let id = match w["paperId"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let authors = w["authors"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["name"].as_str()).map(str::to_string).collect())
            .unwrap_or_default();
        let oa = w["openAccessPdf"]["url"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
        results.push(SearchResult {
            source: "semanticscholar".to_string(),
            external_id: id.clone(),
            doi: w["externalIds"]["DOI"].as_str().and_then(clean_doi),
            title: w["title"].as_str().map(str::to_string),
            authors,
            year: w["year"].as_i64(),
            venue: w["venue"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
            abstract_text: w["abstract"].as_str().map(str::to_string),
            url: Some(format!("https://www.semanticscholar.org/paper/{id}")),
            is_oa: w["isOpenAccess"].as_bool().unwrap_or(false) || oa.is_some(),
            oa_pdf_url: oa,
            citations: w["citationCount"].as_i64().unwrap_or(0),
            in_library: false,
            github_url: None,
            pub_status: None,
        });
    }
    Ok(results)
}

/// Search Europe PMC (EMBL-EBI): biomedical + life sciences, with OA full text.
pub async fn search_europepmc(
    client: &reqwest::Client,
    query: &str,
    filters: &Filters,
) -> Result<Vec<SearchResult>> {
    let mut parts: Vec<String> = Vec::new();
    if !query.trim().is_empty() {
        parts.push(query.trim().to_string());
    }
    if let Some(a) = nonempty(&filters.author) {
        parts.push(format!("AUTH:\"{}\"", a.replace('"', " ")));
    }
    match (filters.year_from, filters.year_to) {
        (Some(f), Some(t)) => parts.push(format!("PUB_YEAR:[{f} TO {t}]")),
        (Some(f), None) => parts.push(format!("PUB_YEAR:[{f} TO 3000]")),
        (None, Some(t)) => parts.push(format!("PUB_YEAR:[0 TO {t}]")),
        (None, None) => {}
    }
    if filters.oa_only {
        parts.push("OPEN_ACCESS:y".to_string());
    }
    if parts.is_empty() {
        return Ok(Vec::new());
    }
    let q = parts.join(" AND ");
    let sort = match filters.sort.as_str() {
        "date" => format!("&sort={}", enc("P_PDATE_D desc")),
        "citations" => format!("&sort={}", enc("CITED desc")),
        _ => String::new(),
    };
    let url = format!(
        "https://www.ebi.ac.uk/europepmc/webservices/rest/search?query={}&format=json&pageSize=40&resultType=core{}",
        enc(&q),
        sort
    );
    let resp = client.get(&url).send().await.context("Europe PMC request")?;
    if !resp.status().is_success() {
        anyhow::bail!("Europe PMC HTTP {}", resp.status());
    }
    let body: Value = resp.json().await.context("Europe PMC JSON")?;
    let mut results = Vec::new();
    for w in body["resultList"]["result"].as_array().cloned().unwrap_or_default() {
        let src = w["source"].as_str().unwrap_or("");
        let pid = w["id"].as_str().unwrap_or("");
        if pid.is_empty() {
            continue;
        }
        let authors = w["authorString"]
            .as_str()
            .map(|s| s.trim_end_matches('.').split(", ").map(str::to_string).collect())
            .unwrap_or_default();
        // OA PDF: prefer an OA-flagged pdf link, else any pdf link.
        let oa = w["fullTextUrlList"]["fullTextUrl"].as_array().and_then(|arr| {
            arr.iter()
                .find(|u| u["documentStyle"].as_str() == Some("pdf") && u["availabilityCode"].as_str() == Some("OA"))
                .or_else(|| arr.iter().find(|u| u["documentStyle"].as_str() == Some("pdf")))
                .and_then(|u| u["url"].as_str())
                .map(str::to_string)
        });
        results.push(SearchResult {
            source: "europepmc".to_string(),
            external_id: format!("{src}:{pid}"),
            doi: w["doi"].as_str().and_then(clean_doi),
            title: w["title"].as_str().map(str::to_string),
            authors,
            year: w["pubYear"].as_str().and_then(|y| y.parse().ok()).or_else(|| w["pubYear"].as_i64()),
            venue: w["journalTitle"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
            abstract_text: w["abstractText"].as_str().map(str::to_string),
            url: Some(format!("https://europepmc.org/article/{src}/{pid}")),
            is_oa: w["isOpenAccess"].as_str() == Some("Y"),
            oa_pdf_url: oa,
            citations: w["citedByCount"].as_i64().unwrap_or(0),
            in_library: false,
            github_url: None,
            pub_status: None,
        });
    }
    Ok(results)
}

/// Search CORE (core.ac.uk): aggregated OA full text. Requires a free API key.
pub async fn search_core(
    client: &reqwest::Client,
    query: &str,
    filters: &Filters,
    api_key: &str,
) -> Result<Vec<SearchResult>> {
    if api_key.trim().is_empty() {
        anyhow::bail!("CORE richiede una API key gratuita (impostala nelle impostazioni)");
    }
    let mut q = query.trim().to_string();
    if let Some(a) = nonempty(&filters.author) {
        q = format!("{q} authors:\"{}\"", a.replace('"', " "));
    }
    if let Some(f) = filters.year_from {
        q = format!("{q} AND yearPublished>={f}");
    }
    if let Some(t) = filters.year_to {
        q = format!("{q} AND yearPublished<={t}");
    }
    if q.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("https://api.core.ac.uk/v3/search/works?q={}&limit=40", enc(q.trim()));
    let resp = client
        .get(&url)
        .bearer_auth(api_key.trim())
        .send()
        .await
        .context("CORE request")?;
    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        anyhow::bail!("CORE: API key non valida");
    }
    if status.as_u16() == 429 {
        anyhow::bail!("CORE: troppe richieste, riprova tra poco");
    }
    if !status.is_success() {
        anyhow::bail!("CORE HTTP {status}");
    }
    let body: Value = resp.json().await.context("CORE JSON")?;
    let mut results = Vec::new();
    for w in body["results"].as_array().cloned().unwrap_or_default() {
        let id = w["id"]
            .as_i64()
            .map(|i| i.to_string())
            .or_else(|| w["id"].as_str().map(str::to_string))
            .unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let authors = w["authors"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["name"].as_str()).map(str::to_string).collect())
            .unwrap_or_default();
        let oa = w["downloadUrl"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
        let doi = w["doi"].as_str().and_then(clean_doi);
        let url = doi
            .as_ref()
            .map(|d| format!("https://doi.org/{d}"))
            .or_else(|| oa.clone());
        results.push(SearchResult {
            source: "core".to_string(),
            external_id: id,
            doi,
            title: w["title"].as_str().map(str::to_string),
            authors,
            year: w["yearPublished"].as_i64(),
            venue: w["publisher"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
            abstract_text: w["abstract"].as_str().map(str::to_string),
            url,
            is_oa: oa.is_some(),
            oa_pdf_url: oa,
            citations: w["citationCount"].as_i64().unwrap_or(0),
            in_library: false,
            github_url: None,
            pub_status: None,
        });
    }
    Ok(results)
}

/// Search DOAJ (Directory of Open Access Journals). Everything is Open Access.
pub async fn search_doaj(
    client: &reqwest::Client,
    query: &str,
    filters: &Filters,
) -> Result<Vec<SearchResult>> {
    let mut q = query.trim().to_string();
    if let Some(a) = nonempty(&filters.author) {
        q = format!("{q} {a}");
    }
    if q.trim().is_empty() {
        return Ok(Vec::new());
    }
    let url = format!("https://doaj.org/api/search/articles/{}?pageSize=40", enc(q.trim()));
    let resp = client.get(&url).send().await.context("DOAJ request")?;
    if !resp.status().is_success() {
        anyhow::bail!("DOAJ HTTP {}", resp.status());
    }
    let body: Value = resp.json().await.context("DOAJ JSON")?;
    let mut results = Vec::new();
    for w in body["results"].as_array().cloned().unwrap_or_default() {
        let id = match w["id"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let b = &w["bibjson"];
        let authors = b["author"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["name"].as_str()).map(str::to_string).collect())
            .unwrap_or_default();
        let doi = b["identifier"]
            .as_array()
            .and_then(|arr| arr.iter().find(|i| i["type"].as_str() == Some("doi")).and_then(|i| i["id"].as_str()))
            .and_then(clean_doi);
        let link = b["link"]
            .as_array()
            .and_then(|arr| {
                arr.iter()
                    .find(|l| l["type"].as_str() == Some("fulltext"))
                    .and_then(|l| l["url"].as_str())
            })
            .map(str::to_string);
        let year = b["year"].as_str().and_then(|y| y.parse::<i64>().ok());
        // DOAJ has no server-side year filter on this endpoint: apply it here.
        if let Some(yy) = year {
            if filters.year_from.is_some_and(|f| yy < f) || filters.year_to.is_some_and(|t| yy > t) {
                continue;
            }
        }
        // Only treat a link as a downloadable PDF when it looks like one.
        let oa_pdf_url = link
            .as_ref()
            .filter(|u| u.to_lowercase().ends_with(".pdf") || u.to_lowercase().contains("/pdf"))
            .cloned();
        results.push(SearchResult {
            source: "doaj".to_string(),
            external_id: id,
            doi,
            title: b["title"].as_str().map(str::to_string),
            authors,
            year,
            venue: b["journal"]["title"].as_str().map(str::to_string),
            abstract_text: b["abstract"].as_str().map(str::to_string),
            url: link,
            is_oa: true,
            oa_pdf_url,
            citations: 0,
            in_library: false,
            github_url: None,
            pub_status: None,
        });
    }
    Ok(results)
}

/// Hugging Face "Daily Papers" — a curated feed of trending ML papers (each tied
/// to an arXiv id). Not a full-text search: the query, if any, filters the feed
/// client-side by title/summary.
pub async fn search_huggingface(
    client: &reqwest::Client,
    query: &str,
    _filters: &Filters,
) -> Result<Vec<SearchResult>> {
    let resp = client
        .get("https://huggingface.co/api/daily_papers?limit=80")
        .send()
        .await
        .context("Hugging Face request")?;
    if !resp.status().is_success() {
        anyhow::bail!("Hugging Face HTTP {}", resp.status());
    }
    let body: Value = resp.json().await.context("Hugging Face JSON")?;
    let terms: Vec<String> = query.to_lowercase().split_whitespace().map(str::to_string).collect();
    let mut results = Vec::new();
    for item in body.as_array().cloned().unwrap_or_default() {
        let p = &item["paper"];
        let aid = p["id"].as_str().unwrap_or("");
        if aid.is_empty() {
            continue;
        }
        let title = p["title"].as_str().or_else(|| item["title"].as_str());
        let summary = p["summary"].as_str();
        // Client-side filter: keep only items matching all query terms (if any).
        if !terms.is_empty() {
            let hay = format!("{} {}", title.unwrap_or(""), summary.unwrap_or("")).to_lowercase();
            if !terms.iter().all(|t| hay.contains(t)) {
                continue;
            }
        }
        let authors = p["authors"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x["name"].as_str()).map(str::to_string).collect())
            .unwrap_or_default();
        let year = p["publishedAt"]
            .as_str()
            .or_else(|| item["publishedAt"].as_str())
            .and_then(year_of);
        results.push(SearchResult {
            source: "huggingface".to_string(),
            external_id: aid.to_string(),
            doi: None,
            title: title.map(str::to_string),
            authors,
            year,
            venue: Some("arXiv · Hugging Face".to_string()),
            abstract_text: summary.map(str::to_string),
            oa_pdf_url: Some(format!("https://arxiv.org/pdf/{aid}")),
            url: Some(format!("https://huggingface.co/papers/{aid}")),
            is_oa: true,
            citations: p["upvotes"].as_i64().unwrap_or(0),
            in_library: false,
            github_url: None,
            pub_status: None,
        });
    }
    Ok(results)
}
