//! Standalone Markdown notes — a lightweight vault of real `.md` files on disk
//! (see `commands::notes_*`). This module holds the pure, testable pieces:
//! deriving a note's title from its body, a short excerpt, and weaving
//! Obsidian-style `[[wikilinks]]` into markdown links resolved by the caller.
//!
//! Rendering (md → sanitized HTML) and slugs are reused verbatim from
//! [`crate::wiki`] so notes and wiki pages share one renderer.

use once_cell::sync::Lazy;
use regex::Regex;

// Inner class excludes `[` and `]` so pathological nested brackets ([[[x]]])
// don't get partially consumed.
static WIKILINK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[\[([^\[\]\n]+)\]\]").unwrap());

/// Update fenced-code-block state for a (leading-trimmed) line. Returns true if
/// the line is a fence delimiter that opens or closes the block (callers skip
/// it). A fence only closes on the SAME marker it opened with (``` vs ~~~), so a
/// different marker inside a block is treated as content — matching CommonMark.
fn track_fence(trimmed: &str, fence: &mut Option<char>) -> bool {
    let marker = if trimmed.starts_with("```") {
        Some('`')
    } else if trimmed.starts_with("~~~") {
        Some('~')
    } else {
        None
    };
    match marker {
        Some(m) => match *fence {
            None => {
                *fence = Some(m);
                true
            }
            Some(open) if open == m => {
                *fence = None;
                true
            }
            Some(_) => false, // other marker inside a fence: content, not a delimiter
        },
        None => false,
    }
}

/// The display title of a note: the first non-empty line of the body, with a
/// leading ATX `#` stripped. Falls back to "Senza titolo".
pub fn note_title(md: &str) -> String {
    let mut fence: Option<char> = None;
    for line in md.lines() {
        let t = line.trim();
        if track_fence(t, &mut fence) || fence.is_some() || t.is_empty() {
            continue;
        }
        let cleaned = t.trim_start_matches('#').trim();
        if !cleaned.is_empty() {
            return truncate_chars(cleaned, 120);
        }
    }
    "Senza titolo".to_string()
}

/// A one-line preview of the note body (past the title line), for the list.
pub fn note_excerpt(md: &str) -> String {
    let mut fence: Option<char> = None;
    let mut seen_title = false;
    let mut parts: Vec<String> = Vec::new();
    for line in md.lines() {
        let t = line.trim();
        if track_fence(t, &mut fence) || fence.is_some() || t.is_empty() {
            continue;
        }
        if !seen_title {
            seen_title = true; // skip the title line
            continue;
        }
        // Collapse an embedded image (e.g. a pasted base64 figure) to a short marker
        // so its data-URI blob doesn't fill the preview; else strip markdown noise.
        let clean = if t.starts_with("![") {
            "[immagine]".to_string()
        } else {
            t.trim_start_matches('#').trim_start_matches('>').trim_start_matches(['-', '*', '+']).trim().to_string()
        };
        if !clean.is_empty() {
            parts.push(clean);
        }
        if parts.join(" ").chars().count() > 160 {
            break;
        }
    }
    truncate_chars(&parts.join(" "), 160)
}

/// Rewrite `[[target]]` (and `[[target|label]]`) wikilinks into markdown links.
/// `resolve(target)` returns `(label, href)` for a hit or `None` to leave the
/// text bare (an unresolved link). Fenced code blocks are left untouched.
pub fn weave_note_links<F>(md: &str, resolve: F) -> String
where
    F: Fn(&str) -> Option<(String, String)>,
{
    let mut out = String::with_capacity(md.len() + 32);
    let mut fence: Option<char> = None;
    for line in md.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if track_fence(trimmed, &mut fence) || fence.is_some() || !line.contains("[[") {
            out.push_str(line);
            continue;
        }
        let replaced = WIKILINK.replace_all(line, |caps: &regex::Captures| {
            let full = caps.get(0).map(|m| m.as_str()).unwrap_or("");
            let raw = caps.get(1).map(|m| m.as_str().trim()).unwrap_or("");
            let (target, explicit_label) = match raw.split_once('|') {
                Some((t, l)) => (t.trim(), Some(l.trim())),
                None => (raw, None),
            };
            if target.is_empty() {
                return full.to_string(); // keep the literal [[ ]] rather than deleting it
            }
            match resolve(target) {
                Some((lbl, href)) => {
                    let text = explicit_label.unwrap_or(&lbl);
                    format!("[{}]({})", clean_label(text), href)
                }
                // Unresolved: show the label (or target) as plain text.
                None => explicit_label.unwrap_or(target).to_string(),
            }
        });
        out.push_str(&replaced);
    }
    out
}

/// Extract every `[[target]]` reference target from a note body (for backlinks).
/// Returns lowercased, trimmed targets (the part before any `|`).
pub fn link_targets(md: &str) -> Vec<String> {
    let mut fence: Option<char> = None;
    let mut out = Vec::new();
    for line in md.lines() {
        let t = line.trim_start();
        if track_fence(t, &mut fence) || fence.is_some() {
            continue;
        }
        for caps in WIKILINK.captures_iter(line) {
            if let Some(m) = caps.get(1) {
                let target = m.as_str().split('|').next().unwrap_or("").trim();
                if !target.is_empty() {
                    out.push(target.to_lowercase());
                }
            }
        }
    }
    out
}

/// Replace the note's title line (the first non-empty, non-fenced line) with
/// `# new_title`. Used by rename. If the note has no content line, the heading
/// is prepended.
pub fn set_title_line(md: &str, new_title: &str) -> String {
    let heading = format!("# {}", new_title.trim());
    // Preserve the note's dominant newline so a hand-authored CRLF file isn't
    // silently converted to LF on rename.
    let nl = if md.contains("\r\n") { "\r\n" } else { "\n" };
    let mut fence: Option<char> = None;
    let mut done = false;
    let mut out: Vec<String> = Vec::new();
    for line in md.lines() {
        if done {
            out.push(line.to_string());
            continue;
        }
        let t = line.trim();
        if track_fence(t, &mut fence) || fence.is_some() || t.is_empty() {
            out.push(line.to_string());
            continue;
        }
        out.push(heading.clone()); // first content line = the title
        done = true;
    }
    if !done {
        return if md.trim().is_empty() {
            format!("{heading}{nl}{nl}")
        } else {
            format!("{heading}{nl}{nl}{md}")
        };
    }
    let mut joined = out.join(nl);
    if md.ends_with('\n') {
        joined.push_str(nl);
    }
    joined
}

/// Keep a link label from breaking the surrounding `[..](..)` markdown.
fn clean_label(s: &str) -> String {
    s.replace('[', "(").replace(']', ")")
}

// ===== Note image assets =====
//
// Pasted/dropped images are stored as real files in `<vault>/assets/` and the
// .md holds only a short `![alt](assets/img-<hash>.<ext>)` reference — a raw
// base64 data-URI would otherwise fill the editor with an unreadable wall of
// text. At render/export time the references are inlined back into data-URIs
// so previews and exports stay self-contained. Filenames are content-addressed
// (FNV-1a of the bytes), so re-extracting the same image is idempotent.

/// Name of the assets subfolder inside the notes vault.
pub const ASSETS_DIR: &str = "assets";
/// Per-image ceiling for embedding/inlining (same cap as the frontend).
pub const ASSET_MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Total inlining budget per note, so a pathological note can't balloon memory.
const INLINE_TOTAL_BUDGET: usize = 100 * 1024 * 1024;

/// FNV-1a 64-bit over the image bytes — a stable content-address for dedup
/// (not security-sensitive; collisions are astronomically unlikely here).
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub fn ext_for_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" | "image/jpg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/bmp" => Some("bmp"),
        "image/svg+xml" => Some("svg"),
        "image/avif" => Some("avif"),
        _ => None,
    }
}

pub fn mime_for_ext(ext: &str) -> Option<&'static str> {
    match ext.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

/// A plain single filename inside assets/ — reject separators and traversal.
fn safe_asset_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 160
        && !name.contains(['/', '\\', ':'])
        && !name.contains("..")
}

/// Unique temp-name suffix so concurrent stores can't interleave on one file.
static TMP_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Write image bytes into `<vault>/assets/` under a content-addressed name and
/// return the relative reference (`assets/img-<hash>.<ext>`) to embed in the .md.
/// The write is atomic (temp + rename) and the dedup check compares the file
/// length, so a partial file from an interrupted write gets repaired instead of
/// being trusted forever.
pub fn store_asset(notes_dir: &std::path::Path, bytes: &[u8], ext: &str) -> Result<String, String> {
    if bytes.len() as u64 > ASSET_MAX_BYTES {
        return Err(format!(
            "Immagine troppo grande ({} MB, max 20)",
            bytes.len() / 1_048_576
        ));
    }
    let dir = notes_dir.join(ASSETS_DIR);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Creazione cartella assets: {e}"))?;
    let filename = format!("img-{:016x}.{ext}", fnv1a(bytes));
    let path = dir.join(&filename);
    let intact = std::fs::metadata(&path)
        .map(|m| m.len() == bytes.len() as u64)
        .unwrap_or(false);
    if !intact {
        let seq = TMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = dir.join(format!("{filename}.{seq}.tmp"));
        std::fs::write(&tmp, bytes).map_err(|e| format!("Salvataggio immagine: {e}"))?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("Salvataggio immagine: {e}")
        })?;
    }
    Ok(format!("{ASSETS_DIR}/{filename}"))
}

/// True if the `](` at `bracket_pos` closes a markdown *image* (`![alt](…)`)
/// rather than a plain link — scan back to the alt's `[` and require a `!`.
fn is_image_ref(text: &str, bracket_pos: usize) -> bool {
    match text[..bracket_pos].rfind('[') {
        Some(0) => false,
        Some(b) => text.as_bytes()[b - 1] == b'!',
        None => false,
    }
}

/// Byte ranges of everything the renderer treats as literal code or math:
/// fenced AND indented code blocks, inline code spans, `$…$` math. The walkers
/// below skip matches inside these ranges, so what they rewrite is exactly what
/// the preview renders as an image — same parser, same options, no disagreement.
fn code_ranges(md: &str) -> Vec<(usize, usize)> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_MATH);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (ev, range) in Parser::new_ext(md, opts).into_offset_iter() {
        match ev {
            Event::Start(Tag::CodeBlock(_)) => {
                if depth == 0 {
                    start = range.start;
                }
                depth += 1;
            }
            Event::End(TagEnd::CodeBlock) => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    out.push((start, range.end));
                }
            }
            Event::Code(_) | Event::InlineMath(_) | Event::DisplayMath(_) => {
                out.push((range.start, range.end));
            }
            _ => {}
        }
    }
    out
}

/// Extract embedded `![alt](data:image/…;base64,…)` images into real files under
/// `<vault>/assets/`, replacing each with a short relative reference. Applied on
/// every save/append (and on open, as a one-time migration), so pasted figures
/// become readable in the editor. Anything the renderer treats as code is left
/// untouched; anything that fails to decode or write stays inline (never lose an
/// image).
pub fn extract_data_images(md: &str, notes_dir: &std::path::Path) -> String {
    const NEEDLE: &str = "](data:image/";
    if !md.contains(NEEDLE) {
        return md.to_string();
    }
    use base64::prelude::{Engine as _, BASE64_STANDARD};
    let codes = code_ranges(md);
    let mut out = String::with_capacity(md.len());
    let mut cursor = 0usize; // everything before `cursor` is already in `out`
    while let Some(rel) = md[cursor..].find(NEEDLE) {
        let pos = cursor + rel; // absolute index of `]`
        let url_start = pos + 2;
        let in_code = codes.iter().any(|&(a, b)| pos >= a && pos < b);
        let close = md[url_start..].find(')').map(|c| url_start + c);
        let parsed = match (close, in_code || !is_image_ref(md, pos)) {
            (Some(c), false) => md[url_start..c]
                .strip_prefix("data:")
                .and_then(|u| u.split_once(";base64,"))
                .and_then(|(mime, payload)| {
                    let ext = ext_for_mime(mime.trim())?;
                    let bytes = BASE64_STANDARD.decode(payload.trim()).ok()?;
                    store_asset(notes_dir, &bytes, ext).ok()
                })
                .map(|r| (c, r)),
            _ => None,
        };
        match parsed {
            Some((c, rel_ref)) => {
                out.push_str(&md[cursor..pos]);
                out.push_str("](");
                out.push_str(&rel_ref);
                out.push(')');
                cursor = c + 1; // past the original `)`
            }
            None => {
                out.push_str(&md[cursor..pos + NEEDLE.len()]);
                cursor = pos + NEEDLE.len();
            }
        }
    }
    out.push_str(&md[cursor..]);
    out
}

/// The inverse, at render/export time: replace `![alt](assets/<name>)` references
/// with `data:` URIs read from disk, so previews and exports are self-contained.
/// Returns the rewritten markdown plus the names of images that SHOULD have been
/// inlined but couldn't (missing/unreadable/oversized/over budget) — exports use
/// it to warn instead of silently shipping broken figures. Unknown extensions and
/// traversal attempts are left as-is and not reported (they never rendered).
pub fn inline_assets(md: &str, notes_dir: &std::path::Path) -> (String, Vec<String>) {
    const NEEDLE: &str = "](assets/";
    if !md.contains(NEEDLE) {
        return (md.to_string(), Vec::new());
    }
    use base64::prelude::{Engine as _, BASE64_STANDARD};
    let dir = notes_dir.join(ASSETS_DIR);
    let codes = code_ranges(md);
    let mut budget = INLINE_TOTAL_BUDGET;
    let mut skipped: Vec<String> = Vec::new();
    let mut out = String::with_capacity(md.len() + 4096);
    let mut cursor = 0usize;
    while let Some(rel) = md[cursor..].find(NEEDLE) {
        let pos = cursor + rel;
        let url_start = pos + 2;
        let in_code = codes.iter().any(|&(a, b)| pos >= a && pos < b);
        let close = md[url_start..].find(')').map(|c| url_start + c);
        let mut expected = false; // a well-formed ref we *should* be able to inline
        let inlined = match (close, in_code || !is_image_ref(md, pos)) {
            (Some(c), false) => {
                let name = &md[url_start + ASSETS_DIR.len() + 1..c];
                Some(name)
                    .filter(|n| safe_asset_name(n))
                    .and_then(|n| {
                        let ext = std::path::Path::new(n).extension()?.to_str()?;
                        let mime = mime_for_ext(ext)?;
                        expected = true;
                        let meta = std::fs::metadata(dir.join(n)).ok()?;
                        if meta.len() > ASSET_MAX_BYTES || meta.len() as usize > budget {
                            return None;
                        }
                        let bytes = std::fs::read(dir.join(n)).ok()?;
                        budget = budget.saturating_sub(bytes.len());
                        Some(format!("data:{mime};base64,{}", BASE64_STANDARD.encode(&bytes)))
                    })
                    .map(|url| (c, url))
                    .or_else(|| {
                        if expected {
                            skipped.push(name.to_string());
                        }
                        None
                    })
            }
            _ => None,
        };
        match inlined {
            Some((c, url)) => {
                out.push_str(&md[cursor..pos]);
                out.push_str("](");
                out.push_str(&url);
                out.push(')');
                cursor = c + 1;
            }
            None => {
                out.push_str(&md[cursor..pos + NEEDLE.len()]);
                cursor = pos + NEEDLE.len();
            }
        }
    }
    out.push_str(&md[cursor..]);
    (out, skipped)
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_from_heading_and_plain() {
        assert_eq!(note_title("# Idee sui trasformatori\n\ntesto"), "Idee sui trasformatori");
        assert_eq!(note_title("prima riga\nseconda"), "prima riga");
        assert_eq!(note_title("\n\n   \n"), "Senza titolo");
        assert_eq!(note_title("```\n# non un titolo\n```\nvero titolo"), "vero titolo");
    }

    #[test]
    fn weave_resolves_and_leaves_bare() {
        let md = "vedi [[Attention]] e [[@vaswani2017]] e [[ignoto]].";
        let out = weave_note_links(md, |t| match t {
            "Attention" => Some(("Attention".into(), "#note-attention".into())),
            "@vaswani2017" => Some(("Vaswani 2017".into(), "#doc-42".into())),
            _ => None,
        });
        assert!(out.contains("[Attention](#note-attention)"));
        assert!(out.contains("[Vaswani 2017](#doc-42)"));
        assert!(out.contains("ignoto")); // bare, no link
        assert!(!out.contains("[[")); // all brackets consumed
    }

    #[test]
    fn weave_explicit_label_and_code_skip() {
        let out = weave_note_links("[[slug|Etichetta]]", |_| Some(("X".into(), "#note-slug".into())));
        assert!(out.contains("[Etichetta](#note-slug)"));
        let code = "```\n[[Attention]]\n```\n";
        assert_eq!(weave_note_links(code, |_| Some(("Y".into(), "#z".into()))), code);
    }

    #[test]
    fn targets_lowercased_outside_code() {
        let t = link_targets("[[Foo]] and [[Bar|x]]\n```\n[[Skip]]\n```");
        assert_eq!(t, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn empty_and_nested_wikilinks() {
        // Whitespace-only link is kept literal, not deleted.
        assert_eq!(weave_note_links("a [[ ]] b", |_| None), "a [[ ]] b");
        // Nested brackets: the inner target `x` is captured (not `[x`); surplus
        // brackets stay literal.
        let out = weave_note_links("[[[x]]]", |t| {
            assert_eq!(t, "x");
            Some(("X".into(), "#note-x".into()))
        });
        assert_eq!(out, "[[X](#note-x)]");
    }

    #[test]
    fn set_title_replaces_first_content_line() {
        assert_eq!(set_title_line("# Vecchio\n\ncorpo", "Nuovo"), "# Nuovo\n\ncorpo");
        // First line is plain text → becomes a heading.
        assert_eq!(set_title_line("vecchio titolo\naltro", "Nuovo"), "# Nuovo\naltro");
        // Leading blank/fence lines are preserved; the first real line is the title.
        assert_eq!(set_title_line("\n```\nx\n```\n# T\n", "Nuovo"), "\n```\nx\n```\n# Nuovo\n");
        // Empty note → heading prepended.
        assert_eq!(set_title_line("   \n", "Nuovo"), "# Nuovo\n\n");
        // CRLF notes keep their line endings.
        assert_eq!(set_title_line("# Old\r\ncorpo\r\n", "Nuovo"), "# Nuovo\r\ncorpo\r\n");
    }

    /// A unique temp vault dir per test (no tempfile dev-dep; best-effort cleanup).
    fn test_vault(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("scriptorium-notes-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn extract_and_inline_roundtrip() {
        let dir = test_vault("roundtrip");
        // "hello" — content doesn't need to be a real PNG, nothing sniffs it.
        let md = "# T\n\n![fig](data:image/png;base64,aGVsbG8=)\n\ntesto\n";
        let out = extract_data_images(md, &dir);
        // The blob became a short assets/ reference…
        assert!(!out.contains("base64,aGVsbG8="), "blob still inline: {out}");
        let rel_start = out.find("](assets/img-").expect("assets ref");
        let rel_end = out[rel_start..].find(')').unwrap() + rel_start;
        let rel = &out[rel_start + 2..rel_end];
        // …backed by a real file with the original bytes.
        assert_eq!(std::fs::read(dir.join(rel)).unwrap(), b"hello");
        // Idempotent: extracting again changes nothing, same content-hash name.
        assert_eq!(extract_data_images(&out, &dir), out);
        // Inlining restores the data-URI for render/export, with nothing skipped.
        let (inlined, skipped) = inline_assets(&out, &dir);
        assert!(inlined.contains("](data:image/png;base64,aGVsbG8=)"), "not inlined: {inlined}");
        assert!(skipped.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn extract_skips_code_links_and_bad_payloads() {
        let dir = test_vault("skips");
        // Inside a fence: untouched.
        let fenced = "```\n![f](data:image/png;base64,aGVsbG8=)\n```\n";
        assert_eq!(extract_data_images(fenced, &dir), fenced);
        // Indented code block and inline code span: also literal code — untouched.
        let indented = "testo\n\n    ![f](data:image/png;base64,aGVsbG8=)\n";
        assert_eq!(extract_data_images(indented, &dir), indented);
        let span = "usa `![f](data:image/png;base64,aGVsbG8=)` così\n";
        assert_eq!(extract_data_images(span, &dir), span);
        // A plain link (not an image) is left alone.
        let link = "[vedi](data:image/png;base64,aGVsbG8=)\n";
        assert_eq!(extract_data_images(link, &dir), link);
        // Unknown mime or broken base64: kept inline, never dropped.
        let bad = "![f](data:image/tiff;base64,aGVsbG8=) ![g](data:image/png;base64,***)\n";
        assert_eq!(extract_data_images(bad, &dir), bad);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn inline_rejects_traversal_reports_missing() {
        let dir = test_vault("traversal");
        std::fs::create_dir_all(dir.join(ASSETS_DIR)).unwrap();
        std::fs::write(dir.join(ASSETS_DIR).join("ok.png"), b"x").unwrap();
        // Traversal, separators, unknown extensions: as-is and NOT reported.
        for md in [
            "![f](assets/../secret.png)",
            "![f](assets/sub/x.png)",
            "![f](assets/ok.exe)",
        ] {
            let (out, skipped) = inline_assets(md, &dir);
            assert_eq!(out, md, "should not inline: {md}");
            assert!(skipped.is_empty(), "should not report: {md}");
        }
        // A missing legitimate ref: as-is but REPORTED (exports warn).
        let (out, skipped) = inline_assets("![f](assets/manca.png)", &dir);
        assert_eq!(out, "![f](assets/manca.png)");
        assert_eq!(skipped, vec!["manca.png".to_string()]);
        // Inside inline code: untouched and not reported, even though it exists.
        let (out, skipped) = inline_assets("il ref `![f](assets/ok.png)` resta testo", &dir);
        assert!(out.contains("`![f](assets/ok.png)`"));
        assert!(skipped.is_empty());
        // The legitimate one inlines.
        assert!(inline_assets("![f](assets/ok.png)", &dir).0.contains("data:image/png;base64,"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mismatched_fence_markers_do_not_close() {
        // A ~~~ line inside a ```-opened block must NOT reopen weaving.
        let md = "```\n~~~\n[[Attention]]\n```\nfuori [[Attention]]";
        let out = weave_note_links(md, |_| Some(("A".into(), "#note-a".into())));
        // Inside the (still-open) fence the link is untouched…
        assert!(out.contains("~~~\n[[Attention]]"));
        // …and only the line after the real closing ``` is woven.
        assert!(out.contains("fuori [A](#note-a)"));
        // note_title also ignores the fenced content.
        assert_eq!(note_title(md), "fuori [[Attention]]");
    }
}
