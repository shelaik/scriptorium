//! Retrieval-augmented generation helpers: split document text into overlapping
//! passages for the "ask your library" engine.

/// Split text into overlapping passages, ~`target` chars each with ~`overlap`
/// chars carried over, on word boundaries. Caps the chunk count so indexing
/// stays fast on very long PDFs.
pub fn chunk_text(text: &str, target: usize, overlap: usize, max_chunks: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut chunks: Vec<String> = Vec::new();
    let mut start = 0usize;
    while start < words.len() && chunks.len() < max_chunks {
        // Grow the window until it reaches the target length (in chars).
        let mut end = start;
        let mut len = 0usize;
        while end < words.len() && len < target {
            len += words[end].len() + 1;
            end += 1;
        }
        chunks.push(words[start..end].join(" "));
        if end >= words.len() {
            break;
        }
        // Move the start back by ~overlap chars, but always make progress.
        let mut ov = 0usize;
        let mut back = end;
        while back > start && ov < overlap {
            back -= 1;
            ov += words[back].len() + 1;
        }
        start = back.max(start + 1);
    }
    chunks
}

/// Cosine similarity between two equal-length vectors.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}
