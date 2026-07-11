//! Reconstruct a tabular grid from positioned PDF words (a "stream"/whitespace
//! table parser — no ruling lines needed) and export it to CSV / Markdown / XLSX.
//! Heuristic by design; messy layouts can be refined with the optional AI pass.

use crate::pdf::PdfWord;
use anyhow::Result;

fn median(mut v: Vec<f32>) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    v[v.len() / 2]
}

/// Reconstruct a row×column grid from words. Rows cluster by vertical position;
/// columns by horizontal whitespace gaps shared across the selection.
pub fn reconstruct(words: &[PdfWord]) -> Vec<Vec<String>> {
    if words.is_empty() {
        return Vec::new();
    }
    let med_h = median(words.iter().map(|w| (w.y1 - w.y0).abs()).collect()).max(0.001);
    let row_tol = med_h * 0.6;
    // Column gap threshold: bigger than an intra-cell space, smaller than a column gap.
    let col_gap = med_h * 1.5;

    // ----- Column bands from the horizontal distribution of all words -----
    let mut by_x: Vec<&PdfWord> = words.iter().collect();
    by_x.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
    let mut bands: Vec<(f32, f32)> = Vec::new();
    for w in &by_x {
        match bands.last_mut() {
            Some(last) if w.x0 <= last.1 + col_gap => last.1 = last.1.max(w.x1),
            _ => bands.push((w.x0, w.x1)),
        }
    }
    let col_of = |cx: f32| -> usize {
        bands
            .iter()
            .position(|(a, b)| cx >= *a - col_gap && cx <= *b + col_gap)
            .unwrap_or_else(|| {
                // Nearest band by center distance (fallback).
                bands
                    .iter()
                    .enumerate()
                    .min_by(|(_, x), (_, y)| {
                        let dx = (cx - (x.0 + x.1) / 2.0).abs();
                        let dy = (cx - (y.0 + y.1) / 2.0).abs();
                        dx.partial_cmp(&dy).unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            })
    };

    // ----- Rows by vertical clustering -----
    let mut by_y: Vec<&PdfWord> = words.iter().collect();
    by_y.sort_by(|a, b| {
        let ay = (a.y0 + a.y1) / 2.0;
        let by = (b.y0 + b.y1) / 2.0;
        ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
    });

    let ncols = bands.len().max(1);
    let mut grid: Vec<Vec<String>> = Vec::new();
    let mut row_cells: Vec<Vec<&PdfWord>> = vec![Vec::new(); ncols];
    let mut last_cy = f32::NAN;

    let push_row = |grid: &mut Vec<Vec<String>>, cells: &mut Vec<Vec<&PdfWord>>| {
        let row: Vec<String> = cells
            .iter_mut()
            .map(|cell| {
                cell.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));
                cell.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
            })
            .collect();
        if row.iter().any(|c| !c.trim().is_empty()) {
            grid.push(row);
        }
        for c in cells.iter_mut() {
            c.clear();
        }
    };

    for w in &by_y {
        let cy = (w.y0 + w.y1) / 2.0;
        if !last_cy.is_nan() && (cy - last_cy).abs() > row_tol {
            push_row(&mut grid, &mut row_cells);
        }
        let ci = col_of((w.x0 + w.x1) / 2.0).min(ncols - 1);
        row_cells[ci].push(w);
        last_cy = cy;
    }
    push_row(&mut grid, &mut row_cells);

    trim_empty_columns(grid)
}

/// Escape a text run so it stays LITERAL in Markdown — the only markup in the
/// rich output is what the extractor itself adds (*…*, `<sup>`…). `<`/`&` become
/// entities so stray angle brackets can't read as HTML.
fn md_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '*' => out.push_str("\\*"),
            '_' => out.push_str("\\_"),
            '`' => out.push_str("\\`"),
            '[' => out.push_str("\\["),
            // The notes renderer enables math ($…$) and strikethrough (~~…~~):
            // dollar amounts and tildes in the SOURCE must stay literal.
            '$' => out.push_str("\\$"),
            '~' => out.push_str("\\~"),
            '<' => out.push_str("&lt;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }
    out
}

/// Keep a line from accidentally becoming Markdown BLOCK markup (heading, quote,
/// list, thematic break / setext underline): neutralize the leading marker.
fn guard_line_start(line: String) -> String {
    let t = line.trim_start();
    let escape = t.starts_with('#')
        || t.starts_with('>')
        || t.starts_with("- ")
        || t.starts_with("+ ")
        // A line made ONLY of dashes/equals would render as a rule or turn the
        // previous line into a setext heading.
        || (!t.is_empty() && (t.chars().all(|c| c == '-') || t.chars().all(|c| c == '=')))
        || {
            let d: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
            // Both CommonMark ordered-list marker forms: "1. " and "1) ".
            !d.is_empty() && (t[d.len()..].starts_with(". ") || t[d.len()..].starts_with(") "))
        };
    if !escape {
        return line;
    }
    // Insert the backslash right before the first non-space char. For "1. " the
    // escape goes before the dot ("1\. "), which is what Markdown needs.
    let pos = line.len() - t.len();
    let mut out = String::with_capacity(line.len() + 1);
    out.push_str(&line[..pos]);
    if t.starts_with(|c: char| c.is_ascii_digit()) {
        let d = t.chars().take_while(|c| c.is_ascii_digit()).count();
        out.push_str(&t[..d]);
        out.push('\\');
        out.push_str(&t[d..]);
    } else {
        out.push('\\');
        out.push_str(t);
    }
    out
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Script {
    Normal,
    Sup,
    Sub,
}

/// Rich variant of [`join_text`]: same line clustering, but words carry their
/// style out as a small Markdown subset — `*italic*`, `**bold**`, and
/// `<sup>`/`<sub>` for the raised/lowered small print of papers (citation
/// markers, footnote signs, chemical formulas). Detection is per line: a word is
/// super/subscript when it is clearly smaller than the line's dominant font AND
/// its baseline sits clearly above/below the line's dominant baseline.
pub fn join_text_rich(words: &[PdfWord]) -> String {
    if words.is_empty() {
        return String::new();
    }
    let med_h = median(words.iter().map(|w| (w.y1 - w.y0).abs()).collect()).max(0.001);
    let row_tol = med_h * 0.6;
    let mut by_y: Vec<&PdfWord> = words.iter().collect();
    by_y.sort_by(|a, b| {
        let ay = (a.y0 + a.y1) / 2.0;
        let by = (b.y0 + b.y1) / 2.0;
        ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut lines: Vec<String> = Vec::new();
    let mut cur: Vec<&PdfWord> = Vec::new();
    let mut last_cy = f32::NAN;
    let flush = |cur: &mut Vec<&PdfWord>, lines: &mut Vec<String>| {
        if cur.is_empty() {
            return;
        }
        cur.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));

        // Dominant font size and baseline of the line's NORMAL text.
        let sizes: Vec<f32> = cur.iter().map(|w| w.size).filter(|s| *s > 0.0).collect();
        let dom = median(sizes);
        let bases: Vec<f32> = cur
            .iter()
            .filter(|w| w.baseline.is_finite() && (dom <= 0.0 || w.size >= 0.85 * dom))
            .map(|w| w.baseline)
            .collect();
        let base_med = if bases.is_empty() { f32::NAN } else { median(bases) };

        let script_of = |w: &PdfWord| -> Script {
            if dom <= 0.0 || w.size <= 0.0 || w.size >= 0.86 * dom {
                return Script::Normal;
            }
            if !w.baseline.is_finite() || !base_med.is_finite() {
                return Script::Normal;
            }
            let d = w.baseline - base_med; // positive = lower on the page
            if d <= -0.18 * dom {
                Script::Sup
            } else if d >= 0.10 * dom {
                Script::Sub
            } else {
                Script::Normal
            }
        };

        // Group consecutive same-style words into runs and wrap each run once.
        // Each part keeps its horizontal extent so the assembly below can decide
        // GEOMETRICALLY whether a style boundary had a space ("word *ital*") or
        // was glued ("H<sub>2</sub>O", "results<sup>12</sup>").
        struct Part {
            text: String,
            x0: f32,
            x1: f32,
            h: f32,
        }
        let mut parts: Vec<Part> = Vec::new();
        let mut run: Vec<&PdfWord> = Vec::new();
        let close = |run: &mut Vec<&PdfWord>, parts: &mut Vec<Part>| {
            if run.is_empty() {
                return;
            }
            // Words inside a run were separated by real whitespace or a wide gap
            // in the source, so a plain space is always right here.
            let text = run.iter().map(|w| md_escape(&w.text)).collect::<Vec<_>>().join(" ");
            let (it, bo, sc) = (run[0].italic, run[0].bold, script_of(run[0]));
            let styled = match (it, bo) {
                (true, true) => format!("***{text}***"),
                (false, true) => format!("**{text}**"),
                (true, false) => format!("*{text}*"),
                (false, false) => text,
            };
            let text = match sc {
                Script::Sup => format!("<sup>{styled}</sup>"),
                Script::Sub => format!("<sub>{styled}</sub>"),
                Script::Normal => styled,
            };
            let x0 = run.first().map(|w| w.x0).unwrap_or(0.0);
            let x1 = run.last().map(|w| w.x1).unwrap_or(0.0);
            let h = median(run.iter().map(|w| (w.y1 - w.y0).abs()).collect());
            parts.push(Part { text, x0, x1, h });
            run.clear();
        };
        for w in cur.iter() {
            if let Some(prev) = run.last() {
                // Break also on a size change (mirrors the word splitter's >20%
                // rule): a size-split fragment that classifies Normal (shallow
                // sub, NaN baseline…) must become its own part, so the GEOMETRIC
                // join below decides glue-vs-space — never an unconditional space.
                let size_broke =
                    prev.size > 0.0 && (w.size - prev.size).abs() > 0.2 * prev.size.max(w.size);
                if prev.italic != w.italic
                    || prev.bold != w.bold
                    || size_broke
                    || script_of(prev) != script_of(w)
                {
                    close(&mut run, &mut parts);
                }
            }
            run.push(w);
        }
        close(&mut run, &mut parts);
        let mut line = String::new();
        let mut prev_x1 = f32::NAN;
        let mut prev_h = 0f32;
        for p in parts {
            if !line.is_empty() {
                // Space only when the gap looks like one (same yardstick family as
                // the word splitter): a glued style change stays glued. EXCEPT when
                // both sides carry asterisk markers — "**a***b*" is ambiguous
                // Markdown (any renderer mis-parses it), so a space wins there.
                let gap = p.x0 - prev_x1;
                let star_clash = line.ends_with('*') && p.text.starts_with('*');
                if !prev_x1.is_nan() && (gap > 0.15 * p.h.max(prev_h) || star_clash) {
                    line.push(' ');
                }
            }
            line.push_str(&p.text);
            prev_x1 = p.x1;
            prev_h = p.h;
        }
        lines.push(guard_line_start(line));
        cur.clear();
    };
    for w in by_y {
        let cy = (w.y0 + w.y1) / 2.0;
        if !last_cy.is_nan() && (cy - last_cy).abs() > row_tol {
            flush(&mut cur, &mut lines);
        }
        cur.push(w);
        last_cy = cy;
    }
    flush(&mut cur, &mut lines);
    lines.join("\n")
}

/// Drop columns that are empty in every row.
pub(crate) fn trim_empty_columns(grid: Vec<Vec<String>>) -> Vec<Vec<String>> {
    if grid.is_empty() {
        return grid;
    }
    let ncols = grid.iter().map(|r| r.len()).max().unwrap_or(0);
    let keep: Vec<bool> = (0..ncols)
        .map(|c| grid.iter().any(|r| r.get(c).map(|s| !s.trim().is_empty()).unwrap_or(false)))
        .collect();
    grid.into_iter()
        .map(|r| {
            r.into_iter()
                .enumerate()
                .filter(|(c, _)| keep.get(*c).copied().unwrap_or(false))
                .map(|(_, s)| s)
                .collect()
        })
        .collect()
}

/// CSV (RFC 4180): quote fields containing comma, quote, or newline.
pub fn to_csv(grid: &[Vec<String>]) -> String {
    let esc = |s: &str| -> String {
        if s.contains([',', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    grid.iter()
        .map(|row| row.iter().map(|c| esc(c)).collect::<Vec<_>>().join(","))
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// GitHub-flavored Markdown table (first row treated as header).
pub fn to_markdown(grid: &[Vec<String>]) -> String {
    if grid.is_empty() {
        return String::new();
    }
    let ncols = grid.iter().map(|r| r.len()).max().unwrap_or(0).max(1);
    let cell = |row: &[String], i: usize| row.get(i).map(|s| s.replace('|', "\\|").replace('\n', " ")).unwrap_or_default();
    let mut out = String::new();
    let row_md = |row: &[String]| {
        (0..ncols).map(|i| format!(" {} ", cell(row, i))).collect::<Vec<_>>().join("|")
    };
    out.push('|');
    out.push_str(&row_md(&grid[0]));
    out.push_str("|\n|");
    out.push_str(&vec![" --- "; ncols].join("|"));
    out.push_str("|\n");
    for row in &grid[1..] {
        out.push('|');
        out.push_str(&row_md(row));
        out.push_str("|\n");
    }
    out
}

/// Write the grid to an .xlsx file.
pub fn to_xlsx(grid: &[Vec<String>], path: &str) -> Result<()> {
    use rust_xlsxwriter::Workbook;
    let mut wb = Workbook::new();
    let sheet = wb.add_worksheet();
    for (r, row) in grid.iter().enumerate() {
        for (c, val) in row.iter().enumerate() {
            sheet.write_string(r as u32, c as u16, val)?;
        }
    }
    wb.save(path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One word on the standard test line (y 0.50..0.51). Baselines/sizes are in
    /// "points" of a notional 1000pt page: normal text size 10, baseline 500.
    #[allow(clippy::too_many_arguments)]
    fn w(text: &str, x0: f32, x1: f32, italic: bool, bold: bool, size: f32, baseline: f32) -> PdfWord {
        PdfWord {
            text: text.into(),
            x0,
            y0: 0.50,
            x1,
            y1: 0.51,
            italic,
            bold,
            size,
            baseline,
        }
    }

    #[test]
    fn rich_text_styles_and_scripts() {
        // "in *vitro*" — italic run separated by a real gap.
        let out = join_text_rich(&[
            w("in", 0.10, 0.13, false, false, 10.0, 500.0),
            w("vitro", 0.135, 0.18, true, false, 10.0, 500.0),
        ]);
        assert_eq!(out, "in *vitro*");
        // Citation superscript glued to the word: smaller + raised.
        let out = join_text_rich(&[
            w("results", 0.10, 0.16, false, false, 10.0, 500.0),
            w("12", 0.1602, 0.17, false, false, 7.0, 496.5),
        ]);
        assert_eq!(out, "results<sup>12</sup>");
        // Chemical subscript, glued on both sides: H2O.
        let out = join_text_rich(&[
            w("H", 0.10, 0.11, false, false, 10.0, 500.0),
            w("2", 0.1101, 0.115, false, false, 7.0, 501.6),
            w("O", 0.1151, 0.125, false, false, 10.0, 500.0),
        ]);
        assert_eq!(out, "H<sub>2</sub>O");
        // Bold.
        let out = join_text_rich(&[w("Teorema", 0.1, 0.16, false, true, 10.0, 500.0)]);
        assert_eq!(out, "**Teorema**");
    }

    #[test]
    fn rich_text_escapes_and_guards() {
        // Markdown specials in the SOURCE stay literal.
        let out = join_text_rich(&[w("a*b_c", 0.10, 0.15, false, false, 10.0, 500.0)]);
        assert_eq!(out, "a\\*b\\_c");
        // `<` can't smuggle HTML; `$`/`~` can't trigger math/strikethrough.
        let out = join_text_rich(&[w("x<y", 0.10, 0.15, false, false, 10.0, 500.0)]);
        assert_eq!(out, "x&lt;y");
        let out = join_text_rich(&[w("$5~x", 0.10, 0.15, false, false, 10.0, 500.0)]);
        assert_eq!(out, "\\$5\\~x");
        // A leading `#` must not become a heading; "1) " must not become a list;
        // a dashes-only line must not become a rule.
        let out = join_text_rich(&[
            w("#", 0.10, 0.11, false, false, 10.0, 500.0),
            w("Intro", 0.13, 0.18, false, false, 10.0, 500.0),
        ]);
        assert!(out.starts_with("\\#"), "leading # escaped: {out}");
        let out = join_text_rich(&[
            w("1)", 0.10, 0.12, false, false, 10.0, 500.0),
            w("primo", 0.13, 0.18, false, false, 10.0, 500.0),
        ]);
        assert!(out.starts_with("1\\)"), "paren list escaped: {out}");
        let out = join_text_rich(&[w("---", 0.10, 0.14, false, false, 10.0, 500.0)]);
        assert!(out.starts_with('\\'), "dash-only line escaped: {out}");
    }

    #[test]
    fn size_split_fragments_rejoin_geometrically() {
        // A glued fragment split ONLY by size (classified Normal: baseline NaN)
        // must NOT gain a spurious space: "results12" stays glued.
        let out = join_text_rich(&[
            w("results", 0.10, 0.16, false, false, 10.0, f32::NAN),
            w("12", 0.1602, 0.17, false, false, 7.0, f32::NAN),
        ]);
        assert_eq!(out, "results12");
        // Glued bold→italic boundary would read "**a***b*" — ambiguous Markdown:
        // a space is inserted deliberately.
        let out = join_text_rich(&[
            w("a", 0.10, 0.12, false, true, 10.0, 500.0),
            w("b", 0.1201, 0.14, true, false, 10.0, 500.0),
        ]);
        assert_eq!(out, "**a** *b*");
    }

    #[test]
    fn rich_text_no_false_scripts() {
        // A caption line entirely in small print: small is DOMINANT there, so
        // nothing should be marked sup/sub.
        let out = join_text_rich(&[
            w("Figura", 0.10, 0.14, false, false, 7.0, 500.0),
            w("1:", 0.145, 0.16, false, false, 7.0, 500.0),
            w("risultati", 0.165, 0.22, false, false, 7.0, 500.0),
        ]);
        assert_eq!(out, "Figura 1: risultati");
        // Same-size words with tiny baseline jitter stay normal.
        let out = join_text_rich(&[
            w("testo", 0.10, 0.14, false, false, 10.0, 500.0),
            w("normale", 0.145, 0.20, false, false, 10.0, 500.3),
        ]);
        assert_eq!(out, "testo normale");
    }
}
