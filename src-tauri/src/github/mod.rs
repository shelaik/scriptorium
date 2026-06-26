//! Find a paper's GitHub repository (from its text) and preview it: repo
//! metadata via the GitHub REST API + a sanitized HTML render of the README.
//! Network-only; gated behind the discovery opt-in at the command layer.

use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GhRepo {
    pub owner: String,
    pub repo: String,
    pub full_name: String,
    pub description: Option<String>,
    pub stars: i64,
    pub language: Option<String>,
    pub license: Option<String>,
    pub url: String,
    pub pushed_at: Option<String>,
}

static GH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)github\.com/([A-Za-z0-9][A-Za-z0-9._-]*)/([A-Za-z0-9][A-Za-z0-9._-]*)")
        .expect("valid github regex")
});

/// Owner/repo paths on github.com that aren't real repositories.
const NON_REPO_OWNERS: &[&str] = &[
    "about", "features", "pricing", "sponsors", "topics", "collections", "marketplace",
    "settings", "notifications", "explore", "search", "login", "join", "orgs", "apps",
];

/// A repo path segment is plausible (rejects obviously-non-repo trailing words).
fn clean_segment(s: &str) -> String {
    let s = s.trim_end_matches(['.', ',', ')', ']', '}', '"', '\'', ';', ':']);
    // Strip a single trailing ".git" suffix (clone URLs), not repeatedly.
    s.strip_suffix(".git").unwrap_or(s).to_string()
}

/// The first GitHub repo URL referenced in free text, if any (cheap, local).
pub fn first_repo_url(text: &str) -> Option<String> {
    find_repos_in_text(text, 1)
        .into_iter()
        .next()
        .map(|(o, r)| format!("https://github.com/{o}/{r}"))
}

/// Find distinct `owner/repo` pairs referenced in free text (order-preserving).
pub fn find_repos_in_text(text: &str, limit: usize) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for caps in GH_RE.captures_iter(text) {
        let owner = clean_segment(&caps[1]);
        let repo = clean_segment(&caps[2]);
        if owner.is_empty() || repo.is_empty() {
            continue;
        }
        if NON_REPO_OWNERS.contains(&owner.to_ascii_lowercase().as_str()) {
            continue;
        }
        // Common false positives where the second segment is a page, not a repo.
        if matches!(repo.to_ascii_lowercase().as_str(), "blob" | "tree" | "releases" | "issues" | "wiki" | "actions") {
            continue;
        }
        let key = (owner.to_ascii_lowercase(), repo.to_ascii_lowercase());
        if out.iter().any(|(o, r)| o.to_ascii_lowercase() == key.0 && r.to_ascii_lowercase() == key.1) {
            continue;
        }
        out.push((owner, repo));
        if out.len() >= limit {
            break;
        }
    }
    out
}

fn auth(req: reqwest::RequestBuilder, token: &str) -> reqwest::RequestBuilder {
    let req = req.header("Accept", "application/vnd.github+json");
    if token.trim().is_empty() {
        req
    } else {
        req.header("Authorization", format!("Bearer {}", token.trim()))
    }
}

/// Fetch repository metadata (None if it doesn't exist / is private / rate-limited).
pub async fn fetch_repo(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Option<GhRepo> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}");
    let resp = auth(client.get(&url), token).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    Some(GhRepo {
        owner: owner.to_string(),
        repo: repo.to_string(),
        full_name: v["full_name"].as_str().unwrap_or(&format!("{owner}/{repo}")).to_string(),
        description: v["description"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        stars: v["stargazers_count"].as_i64().unwrap_or(0),
        language: v["language"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
        license: v["license"]["spdx_id"]
            .as_str()
            .filter(|s| !s.is_empty() && *s != "NOASSERTION")
            .map(str::to_string),
        url: v["html_url"].as_str().unwrap_or(&url).to_string(),
        pushed_at: v["pushed_at"].as_str().map(|s| s.chars().take(10).collect()),
    })
}

/// Fetch a repo's README and render it to sanitized HTML (safe for `{@html}`).
pub async fn fetch_readme_html(
    client: &reqwest::Client,
    token: &str,
    owner: &str,
    repo: &str,
) -> Result<String> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/readme");
    let resp = auth(client.get(&url), token)
        .header("Accept", "application/vnd.github.raw")
        .send()
        .await
        .context("GitHub README request")?;
    let status = resp.status();
    if status.as_u16() == 404 {
        anyhow::bail!("Questo repository non ha un README");
    }
    if status.as_u16() == 403 {
        anyhow::bail!("GitHub: limite di richieste raggiunto (imposta un token nelle impostazioni)");
    }
    if !status.is_success() {
        anyhow::bail!("GitHub HTTP {status}");
    }
    let markdown = resp.text().await.context("GitHub README body")?;
    Ok(render_markdown(&markdown))
}

/// Markdown -> HTML (GitHub-ish options) -> sanitized HTML.
fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(md, opts);
    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, parser);
    // ammonia strips scripts/styles/event handlers and unsafe URLs by default.
    ammonia::clean(&unsafe_html)
}
