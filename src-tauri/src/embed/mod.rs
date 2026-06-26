//! Local text embeddings via fastembed (bge-m3, 1024-dim, multilingual).
//!
//! The model (~2.3GB) is downloaded into the cache dir on first use and reused
//! afterwards. The embedder is initialized lazily and guarded by a mutex
//! because `TextEmbedding::embed` takes `&mut self`.

use anyhow::{anyhow, Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use once_cell::sync::OnceCell;
use std::path::Path;
use std::sync::Mutex;

/// bge-m3 dense embedding dimension — must match the `doc_vec` vec0 table.
pub const EMBED_DIM: usize = 1024;

/// How many characters of fulltext to fold into a document's embedding text.
const FULLTEXT_CHARS: usize = 2000;

static EMBEDDER: OnceCell<Mutex<TextEmbedding>> = OnceCell::new();

/// Lazily initialize the bge-m3 embedder. The first call downloads the model
/// into `cache_dir`; later calls reuse the cached files.
fn embedder(cache_dir: &Path) -> Result<&'static Mutex<TextEmbedding>> {
    EMBEDDER.get_or_try_init(|| {
        std::fs::create_dir_all(cache_dir).ok();
        let opts = TextInitOptions::new(EmbeddingModel::BGEM3)
            .with_cache_dir(cache_dir.to_path_buf())
            .with_show_download_progress(true);
        let model = TextEmbedding::try_new(opts).context("initializing the bge-m3 embedder")?;
        Ok::<_, anyhow::Error>(Mutex::new(model))
    })
}

/// Compose the text used to represent a document in the embedding space:
/// title + abstract + a prefix of the fulltext.
pub fn compose_text(title: Option<&str>, abstract_: Option<&str>, fulltext: &str) -> String {
    let mut s = String::new();
    if let Some(t) = title {
        s.push_str(t);
        s.push('\n');
    }
    if let Some(a) = abstract_ {
        s.push_str(a);
        s.push('\n');
    }
    let prefix: String = fulltext.chars().take(FULLTEXT_CHARS).collect();
    s.push_str(&prefix);
    s
}

/// Embed a batch of texts into 1024-dim dense vectors.
pub fn embed_batch(cache_dir: &Path, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
    let m = embedder(cache_dir)?;
    let mut guard = m.lock().map_err(|_| anyhow!("embedder mutex poisoned"))?;
    let n = texts.len();
    let vectors = guard.embed(texts, None).context("embedding texts")?;
    // Must get exactly one vector per input (else the caller's id/vector zip truncates).
    if vectors.len() != n {
        return Err(anyhow!(
            "embedder returned {} vectors for {n} texts",
            vectors.len()
        ));
    }
    // Guard against a dimension mismatch with the vec0 table.
    if let Some(v) = vectors.first() {
        if v.len() != EMBED_DIM {
            return Err(anyhow!(
                "unexpected embedding dimension {} (expected {EMBED_DIM})",
                v.len()
            ));
        }
    }
    Ok(vectors)
}

/// Embed a single query string.
pub fn embed_query(cache_dir: &Path, text: &str) -> Result<Vec<f32>> {
    embed_batch(cache_dir, vec![text.to_string()])?
        .pop()
        .context("no embedding produced for query")
}
