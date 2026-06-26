//! Minimal BibTeX parser — enough to import a Zotero/Mendeley `.bib` export into
//! reference-only items. Not a full BibTeX engine: it reads `@type{key, field =
//! {value} | "value" | value, ...}` entries, handles nested braces, and ignores
//! `@comment`/`@string`/`@preamble`.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct BibEntry {
    pub entry_type: String,
    pub key: String,
    pub fields: HashMap<String, String>,
}

/// Parse a BibTeX document into entries.
pub fn parse(input: &str) -> Vec<BibEntry> {
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    let n = chars.len();
    let mut out = Vec::new();

    while i < n {
        // Seek the next '@'.
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        i += 1; // past '@'
        // entry type
        let mut etype = String::new();
        while i < n && (chars[i].is_alphanumeric() || chars[i] == '_') {
            etype.push(chars[i]);
            i += 1;
        }
        let etype_l = etype.to_ascii_lowercase();
        // skip whitespace
        while i < n && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= n || chars[i] != '{' {
            continue;
        }
        i += 1; // past '{'
        // Non-reference entry types: skip to the matching close brace.
        if matches!(etype_l.as_str(), "comment" | "string" | "preamble") {
            i = skip_to_close(&chars, i);
            continue;
        }
        // citation key (up to first comma)
        let mut key = String::new();
        while i < n && chars[i] != ',' && chars[i] != '}' {
            key.push(chars[i]);
            i += 1;
        }
        let mut entry = BibEntry {
            entry_type: etype_l,
            key: key.trim().to_string(),
            fields: HashMap::new(),
        };
        // fields
        while i < n && chars[i] != '}' {
            if chars[i] == ',' || chars[i].is_whitespace() {
                i += 1;
                continue;
            }
            // field name
            let mut name = String::new();
            while i < n && chars[i] != '=' && chars[i] != '}' && !chars[i].is_whitespace() {
                name.push(chars[i]);
                i += 1;
            }
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            if i >= n || chars[i] != '=' {
                // Malformed; bail out of this entry.
                break;
            }
            i += 1; // past '='
            while i < n && chars[i].is_whitespace() {
                i += 1;
            }
            // field value
            let value = if i < n && chars[i] == '{' {
                i += 1;
                read_braced(&chars, &mut i)
            } else if i < n && chars[i] == '"' {
                i += 1;
                read_quoted(&chars, &mut i)
            } else {
                // bare token (number / macro)
                let mut v = String::new();
                while i < n && chars[i] != ',' && chars[i] != '}' {
                    v.push(chars[i]);
                    i += 1;
                }
                v.trim().to_string()
            };
            if !name.trim().is_empty() {
                entry.fields.insert(name.trim().to_ascii_lowercase(), clean_value(&value));
            }
        }
        if i < n && chars[i] == '}' {
            i += 1;
        }
        if !entry.fields.is_empty() {
            out.push(entry);
        }
    }
    out
}

/// Skip from just-after an opening brace to just-after its matching close.
fn skip_to_close(chars: &[char], mut i: usize) -> usize {
    let mut depth = 1;
    while i < chars.len() && depth > 0 {
        match chars[i] {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    i
}

/// Read a `{...}` value (already past the opening brace); consumes the closing brace.
fn read_braced(chars: &[char], i: &mut usize) -> String {
    let mut depth = 1;
    let mut s = String::new();
    while *i < chars.len() {
        let c = chars[*i];
        match c {
            '{' => {
                depth += 1;
                s.push(c);
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    *i += 1;
                    break;
                }
                s.push(c);
            }
            _ => s.push(c),
        }
        *i += 1;
    }
    s
}

/// Read a `"..."` value (already past the opening quote); allows `{...}` nesting.
fn read_quoted(chars: &[char], i: &mut usize) -> String {
    let mut s = String::new();
    let mut depth = 0;
    while *i < chars.len() {
        let c = chars[*i];
        if c == '{' {
            depth += 1;
        } else if c == '}' && depth > 0 {
            depth -= 1;
        } else if c == '"' && depth == 0 {
            *i += 1;
            break;
        }
        s.push(c);
        *i += 1;
    }
    s
}

/// Light LaTeX/brace cleanup for a field value (not a full de-TeX).
fn clean_value(s: &str) -> String {
    let mut out = s.replace('\n', " ");
    for (from, to) in [("\\&", "&"), ("--", "–"), ("``", "\""), ("''", "\""), ("\\%", "%")] {
        out = out.replace(from, to);
    }
    out.retain(|c| c != '{' && c != '}');
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Split a BibTeX `author`/`editor` field ("A and B and C") into (given, family) pairs.
pub fn split_authors(field: &str) -> Vec<(Option<String>, Option<String>)> {
    field
        .split(" and ")
        .map(|name| {
            let name = name.trim();
            if let Some((family, given)) = name.split_once(',') {
                (
                    non_empty(given.trim()),
                    non_empty(family.trim()),
                )
            } else if let Some((given, family)) = name.rsplit_once(' ') {
                (non_empty(given.trim()), non_empty(family.trim()))
            } else {
                (None, non_empty(name))
            }
        })
        .filter(|(g, f)| g.is_some() || f.is_some())
        .collect()
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}
