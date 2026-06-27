//! Serializable data types shared across the Tauri command boundary.

use serde::Serialize;

/// A colored label that can be attached to documents.
#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub id: i64,
    pub name: String,
    pub color: Option<String>,
}

/// A library entry as shown in the UI. Metadata fields are filled at import
/// time where possible (title defaults to the filename) and enriched later by
/// the Crossref/OpenAlex lookup.
#[derive(Debug, Clone, Serialize)]
pub struct Document {
    pub id: i64,
    pub title: Option<String>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    pub authors: Vec<String>,
    pub tags: Vec<Tag>,
    /// Whether a cached thumbnail exists (fetch its bytes via `get_thumbnail`).
    pub has_thumb: bool,
    pub added_at: Option<String>,
    pub is_read: bool,
    pub favorite: bool,
    /// First GitHub repo URL found in the document's text, if any.
    pub github_url: Option<String>,
    /// "published" | "preprint" | "preprint_reviewed" | null.
    pub pub_status: Option<String>,
    /// Best original link for sharing (DOI, else arXiv abstract page).
    pub paper_url: Option<String>,
    /// Persistent, library-unique citation key (see `db::citekey`).
    pub citekey: Option<String>,
}

/// Full editable metadata for one document (used by the manual editor).
#[derive(Debug, Clone, Serialize)]
pub struct EditableMeta {
    pub title: Option<String>,
    pub authors: Vec<String>,
    pub year: Option<i64>,
    pub venue: Option<String>,
    pub doi: Option<String>,
    pub abstract_text: Option<String>,
    pub notes: Option<String>,
    pub summary: Option<String>,
}

/// A collection of documents: manual (membership table) or smart (rule_json).
#[derive(Debug, Clone, Serialize)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub is_smart: bool,
    pub rule_json: Option<String>,
}

/// A highlight/note anchored to a page region (rects normalized to 0..1).
#[derive(Debug, Clone, Serialize)]
pub struct Annotation {
    pub id: i64,
    pub page: i64,
    /// `highlight` | `underline` | `strikethrough` | `note`.
    pub kind: String,
    pub color: Option<String>,
    pub rects_json: String,
    pub quote: Option<String>,
    pub note: Option<String>,
    pub created_at: Option<String>,
}

/// Summary of an import batch, returned to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct ImportSummary {
    /// Document ids that were newly imported.
    pub imported: Vec<i64>,
    /// Document ids that already existed (duplicates, by hash or path).
    pub duplicates: Vec<i64>,
    /// Human-readable errors for files that failed to import.
    pub errors: Vec<String>,
    /// Soft warnings (e.g. imported but text/thumbnail could not be extracted).
    pub warnings: Vec<String>,
}
