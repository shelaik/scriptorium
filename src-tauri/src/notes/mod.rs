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
