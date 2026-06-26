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

/// Join region words into readable prose: cluster into lines by Y, order each
/// line left→right, join words by space and lines by newline.
pub fn join_text(words: &[PdfWord]) -> String {
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
        lines.push(cur.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" "));
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
fn trim_empty_columns(grid: Vec<Vec<String>>) -> Vec<Vec<String>> {
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
