//! mirror — lo «Specchio su disco» dell'Archivio.
//!
//! Proietta le raccolte in una cartella leggibile dall'utente:
//!     <specchio>\Raccolta\Sottoraccolta\Autore Anno — Titolo.pdf
//! usando HARDLINK NTFS (stesso volume: zero byte duplicati; un paper in più
//! raccolte appare in più cartelle gratis) con fallback a copia vera quando
//! l'hardlink non è possibile (altro volume).
//!
//! Sicuro per costruzione:
//! - la libreria vera (papers/, DB) NON viene mai toccata: lo specchio è una
//!   proiezione one-way, rigenerabile da zero in ogni momento;
//! - la pulizia avviene SOLO dentro una cartella che contiene il file marker
//!   `.scriptorium-mirror` (mai svuotare per errore una cartella qualunque);
//! - i nomi di cartelle/file sono sanitizzati (separatori, `..`, riservati
//!   Windows, lunghezze) e il percorso finale deve restare sotto la radice.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

/// Serializza le rigenerazioni: il bottone RIGENERA e il sync debounced non
/// devono mai pulire/ricostruire lo specchio uno sotto i piedi dell'altro.
static REGEN_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

pub const MARKER: &str = ".scriptorium-mirror";
const MARKER_BODY: &str = "Cartella generata da Scriptorium (Archivio → Specchio su disco).\n\
CANCELLARE, rinominare o spostare file qui dentro NON tocca la libreria,\n\
e la cartella viene rigenerata automaticamente (le modifiche non durano).\n\
ATTENZIONE però: i file sono HARDLINK — lo STESSO file della libreria con un\n\
altro nome. MODIFICARNE IL CONTENUTO (es. annotare il PDF e salvare) modifica\n\
anche la copia in libreria. Per annotare in sicurezza usa il lettore di\n\
Scriptorium, o fai prima una copia altrove.\n";

/// Riepilogo di una rigenerazione.
#[derive(serde::Serialize, Default)]
pub struct MirrorSummary {
    pub folders: usize,
    pub linked: usize,
    pub copied: usize,
    pub missing: usize,
}

#[derive(serde::Serialize)]
pub struct MirrorStatus {
    pub enabled: bool,
    pub dir: String,
}

/// Nome file/cartella sicuro per NTFS: via separatori e traversal, niente nomi
/// riservati, niente code di punti/spazi, lunghezza limitata.
pub fn sanitize_component(name: &str) -> String {
    let mut s: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            c if (c as u32) < 0x20 => ' ',
            c => c,
        })
        .collect();
    s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    while s.ends_with('.') || s.ends_with(' ') {
        s.pop();
    }
    if s.len() > 90 {
        let mut cut = 90;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
        while s.ends_with('.') || s.ends_with(' ') {
            s.pop();
        }
    }
    let upper = s.to_ascii_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if s.is_empty() || s == "." || s == ".." || RESERVED.contains(&upper.as_str()) {
        return format!("_{s}");
    }
    s
}

fn setting(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0))
        .ok()
}

pub fn status(app: &tauri::AppHandle) -> MirrorStatus {
    let state = app.state::<crate::AppState>();
    let conn = state.db.lock();
    MirrorStatus {
        enabled: setting(&conn, "mirror_enabled").as_deref() == Some("1"),
        dir: setting(&conn, "mirror_dir").unwrap_or_default(),
    }
}

/// Valida la cartella scelta: creabile, non dentro i dati dell'app, non una
/// radice di volume; se non è vuota deve già essere un nostro specchio (marker).
fn validate_dir(app: &tauri::AppHandle, dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("Non riesco a creare la cartella: {e}"))?;
    let canon = std::fs::canonicalize(dir).map_err(|e| e.to_string())?;
    if canon.parent().is_none() {
        return Err("Non usare la radice di un disco: scegli una sottocartella dedicata".into());
    }
    let data_dir = app
        .path()
        .app_data_dir()
        .ok()
        .and_then(|d| std::fs::canonicalize(d).ok());
    if let Some(data) = data_dir {
        if canon == data || data.starts_with(&canon) || canon.starts_with(&data) {
            return Err("Scegli una cartella FUORI dai dati dell'app (%APPDATA%\\com.pdfmanage.app)".into());
        }
    }
    // Mai dentro (o uguale a) la cartella sorvegliata: ogni sync farebbe
    // scattare il watcher in un ciclo di re-import rumorosi.
    let watched = {
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock();
        setting(&conn, "watched_folder")
    };
    if let Some(w) = watched {
        if let Ok(wc) = std::fs::canonicalize(&w) {
            if canon == wc || canon.starts_with(&wc) || wc.starts_with(&canon) {
                return Err("Scegli una cartella separata dalla cartella sorvegliata (il watcher re-importerebbe lo specchio in loop)".into());
            }
        }
    }
    let has_marker = canon.join(MARKER).is_file();
    let non_empty = std::fs::read_dir(&canon)
        .map(|mut it| it.next().is_some())
        .unwrap_or(false);
    if non_empty && !has_marker {
        return Err("La cartella non è vuota e non è uno specchio di Scriptorium: scegline una vuota o dedicata".into());
    }
    std::fs::write(canon.join(MARKER), MARKER_BODY).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn set_enabled(app: &tauri::AppHandle, enabled: bool, dir: Option<String>) -> Result<MirrorStatus, String> {
    if enabled {
        let dir_s = {
            let state = app.state::<crate::AppState>();
            let conn = state.db.lock();
            dir.clone().or_else(|| setting(&conn, "mirror_dir")).unwrap_or_default()
        };
        if dir_s.trim().is_empty() {
            return Err("Scegli prima la cartella dello specchio".into());
        }
        validate_dir(app, Path::new(&dir_s))?;
        // Volume diverso dalla libreria → niente hardlink: copie vere (spazio
        // doppio, sync più lenti). Funziona, ma l'utente deve saperlo.
        let drive = |p: &Path| p.components().next().map(|c| c.as_os_str().to_ascii_uppercase());
        let lib_drive = app.path().app_data_dir().ok().and_then(|d| drive(&d));
        if lib_drive.is_some() && drive(Path::new(&dir_s)) != lib_drive {
            crate::pulse::warn(
                app,
                "archivio",
                "Specchio su un altro volume",
                "niente hardlink possibili: verranno fatte copie vere (spazio doppio)",
            );
        }
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('mirror_dir', ?1)",
            [dir_s.as_str()],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('mirror_enabled', '1')",
            [],
        )
        .map_err(|e| e.to_string())?;
    } else {
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock();
        conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES ('mirror_enabled', '0')",
            [],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(status(app))
}

/// Una voce da proiettare: cartella relativa (già sanitizzata) + sorgente + nome file.
struct Entry {
    rel_dir: PathBuf,
    src: PathBuf,
    file_name: String,
}

fn doc_file_name(title: Option<&str>, year: Option<i64>, lead: Option<&str>, id: i64) -> String {
    let mut base = String::new();
    if let Some(l) = lead {
        if !l.trim().is_empty() {
            base.push_str(l.trim());
            base.push(' ');
        }
    }
    if let Some(y) = year {
        base.push_str(&y.to_string());
        base.push_str(" — ");
    }
    match title {
        Some(t) if !t.trim().is_empty() => base.push_str(t.trim()),
        _ => base.push_str(&format!("Documento {id}")),
    }
    format!("{}.pdf", sanitize_component(&base))
}

/// Rigenerazione completa: pulisce lo specchio (solo se marcato) e lo ricrea.
pub fn regenerate(app: &tauri::AppHandle) -> Result<MirrorSummary, String> {
    let _serialize = REGEN_LOCK.lock();
    let st = status(app);
    if !st.enabled {
        return Err("Lo specchio è spento (attivalo nell'Archivio)".into());
    }
    if st.dir.trim().is_empty() {
        return Err("Nessuna cartella scelta per lo specchio".into());
    }
    let root = PathBuf::from(&st.dir);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let root = std::fs::canonicalize(&root).map_err(|e| e.to_string())?;
    if !root.join(MARKER).is_file() {
        return Err("La cartella dello specchio non ha il marker di Scriptorium: riattivalo dall'Archivio".into());
    }

    // 1) Piano delle voci, sotto lock breve.
    let entries: Vec<Entry> = {
        let state = app.state::<crate::AppState>();
        let conn = state.db.lock();
        // Percorso di ogni raccolta (id -> cartella relativa sanitizzata).
        let mut cols: Vec<(i64, String, Option<i64>)> = Vec::new();
        {
            let mut stmt = conn
                .prepare("SELECT id, name, parent_id FROM collections WHERE is_smart = 0")
                .map_err(|e| e.to_string())?;
            let it = stmt
                .query_map([], |r| Ok((r.get(0)?, r.get::<_, String>(1)?, r.get(2)?)))
                .map_err(|e| e.to_string())?;
            cols.extend(it.filter_map(Result::ok));
        }
        let by_id: std::collections::HashMap<i64, (String, Option<i64>)> =
            cols.iter().map(|(id, n, p)| (*id, (n.clone(), *p))).collect();
        let rel_of = |id: i64| -> PathBuf {
            let mut parts: Vec<String> = Vec::new();
            let mut cur = Some(id);
            // Stesso tetto di move_collection (64): mai troncare in silenzio
            // un albero legale ri-radicando le cartelle a metà.
            for _ in 0..64 {
                let Some(c) = cur else { break };
                let Some((name, parent)) = by_id.get(&c) else { break };
                parts.push(sanitize_component(name));
                cur = *parent;
            }
            parts.reverse();
            parts.iter().collect()
        };

        let mut out: Vec<Entry> = Vec::new();
        // Membri delle raccolte (documenti vivi con un PDF vero su disco).
        {
            let mut stmt = conn
                .prepare(
                    "SELECT cm.collection_id, d.id, d.path, d.title, d.year,
                            (SELECT a.family FROM authors a JOIN document_authors da ON da.author_id = a.id
                             WHERE da.document_id = d.id ORDER BY da.position LIMIT 1)
                     FROM collection_members cm
                     JOIN documents d ON d.id = cm.document_id
                     JOIN collections c ON c.id = cm.collection_id AND c.is_smart = 0
                     WHERE d.deleted_at IS NULL AND d.path NOT LIKE 'ref:%'",
                )
                .map_err(|e| e.to_string())?;
            let it = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, Option<i64>>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            for (cid, id, path, title, year, lead) in it.filter_map(Result::ok) {
                out.push(Entry {
                    rel_dir: rel_of(cid),
                    src: PathBuf::from(path),
                    file_name: doc_file_name(title.as_deref(), year, lead.as_deref(), id),
                });
            }
        }
        // Senza raccolta.
        {
            let mut stmt = conn
                .prepare(
                    "SELECT d.id, d.path, d.title, d.year,
                            (SELECT a.family FROM authors a JOIN document_authors da ON da.author_id = a.id
                             WHERE da.document_id = d.id ORDER BY da.position LIMIT 1)
                     FROM documents d
                     WHERE d.deleted_at IS NULL AND d.path NOT LIKE 'ref:%'
                       AND d.id NOT IN (SELECT document_id FROM collection_members)",
                )
                .map_err(|e| e.to_string())?;
            let it = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, Option<String>>(2)?,
                        r.get::<_, Option<i64>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                    ))
                })
                .map_err(|e| e.to_string())?;
            for (id, path, title, year, lead) in it.filter_map(Result::ok) {
                out.push(Entry {
                    rel_dir: PathBuf::from("Senza raccolta"),
                    src: PathBuf::from(path),
                    file_name: doc_file_name(title.as_deref(), year, lead.as_deref(), id),
                });
            }
        }
        out
    };

    // 2) Pulizia (solo contenuto, marker escluso) — la cartella è marcata.
    for e in std::fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
        let p = e.path();
        if p.file_name().and_then(|n| n.to_str()) == Some(MARKER) {
            continue;
        }
        let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
    }

    // 3) Ricostruzione.
    let mut sum = MirrorSummary::default();
    let mut made_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    let mut used: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for e in entries {
        if !e.src.is_file() {
            sum.missing += 1;
            continue;
        }
        let dir = root.join(&e.rel_dir);
        // Cintura: il percorso composto deve restare sotto la radice.
        if !dir.starts_with(&root) {
            sum.missing += 1;
            continue;
        }
        if made_dirs.insert(dir.clone()) {
            if std::fs::create_dir_all(&dir).is_err() {
                sum.missing += 1;
                continue;
            }
            sum.folders += 1;
        }
        // Collisioni di nome nella stessa cartella: suffisso (2), (3)…
        let mut target = dir.join(&e.file_name);
        let mut k = 2;
        while used.contains(&target) {
            let stem = e.file_name.trim_end_matches(".pdf");
            target = dir.join(format!("{stem} ({k}).pdf"));
            k += 1;
        }
        used.insert(target.clone());
        match std::fs::hard_link(&e.src, &target) {
            Ok(()) => sum.linked += 1,
            Err(_) => match std::fs::copy(&e.src, &target) {
                Ok(_) => sum.copied += 1,
                Err(_) => sum.missing += 1,
            },
        }
    }
    Ok(sum)
}

// ---- sync automatico (debounced) -------------------------------------------

static SYNC_PENDING: AtomicBool = AtomicBool::new(false);

/// Chiedi una rigenerazione fra ~2s (le richieste ravvicinate collassano in una).
pub fn request_sync(app: &tauri::AppHandle) {
    if status(app).enabled != true {
        return;
    }
    if SYNC_PENDING.swap(true, Ordering::SeqCst) {
        return; // già in coda
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        SYNC_PENDING.store(false, Ordering::SeqCst);
        // Spento durante l'attesa? Esci in silenzio: l'utente l'ha appena
        // voluto spegnere, un warn qui sarebbe solo rumore.
        if !status(&app).enabled {
            return;
        }
        let app2 = app.clone();
        let r = tauri::async_runtime::spawn_blocking(move || regenerate(&app2)).await;
        match r {
            Ok(Ok(s)) => crate::pulse::blip(
                &app,
                "archivio",
                &format!("Specchio aggiornato: {} link, {} copie, {} cartelle", s.linked, s.copied, s.folders),
            ),
            Ok(Err(e)) => crate::pulse::warn(&app, "archivio", "Specchio su disco", &e),
            Err(e) => crate::pulse::warn(&app, "archivio", "Specchio su disco", &e.to_string()),
        }
    });
}

// ---- comandi ----------------------------------------------------------------

#[tauri::command]
pub fn mirror_status(app: tauri::AppHandle) -> MirrorStatus {
    status(&app)
}

#[tauri::command]
pub fn set_mirror(app: tauri::AppHandle, enabled: bool, dir: Option<String>) -> Result<MirrorStatus, String> {
    let st = set_enabled(&app, enabled, dir)?;
    if st.enabled {
        request_sync(&app);
    }
    Ok(st)
}

#[tauri::command]
pub async fn mirror_regenerate(app: tauri::AppHandle) -> Result<MirrorSummary, String> {
    crate::pulse::start(&app, "archivio", "Specchio su disco: rigenerazione");
    let app2 = app.clone();
    let r = tauri::async_runtime::spawn_blocking(move || regenerate(&app2))
        .await
        .map_err(|e| e.to_string())
        .and_then(|x| x);
    match &r {
        Ok(s) => crate::pulse::ok(
            &app,
            "archivio",
            "Specchio su disco",
            &format!("{} hardlink, {} copie, {} cartelle, {} mancanti", s.linked, s.copied, s.folders, s.missing),
        ),
        Err(e) => crate::pulse::err(&app, "archivio", "Specchio su disco", e),
    }
    r
}

#[tauri::command]
pub fn mirror_reveal(app: tauri::AppHandle) -> Result<(), String> {
    let st = status(&app);
    if st.dir.trim().is_empty() {
        return Err("Nessuna cartella scelta".into());
    }
    let explorer = std::env::var_os("WINDIR")
        .map(|w| PathBuf::from(w).join("explorer.exe"))
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\explorer.exe"));
    std::process::Command::new(explorer)
        .arg(&st.dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_separators_and_traversal() {
        assert_eq!(sanitize_component("a/b\\c"), "a b c");
        // I punti in coda vengono strippati PRIMA del check: ".." collassa a "" → "_".
        assert_eq!(sanitize_component(".."), "_");
        assert_eq!(sanitize_component("CON"), "_CON");
        assert_eq!(sanitize_component("fine."), "fine");
        assert_eq!(sanitize_component("  spazi   doppi  "), "spazi doppi");
        assert_eq!(sanitize_component(""), "_");
    }

    #[test]
    fn sanitize_caps_length_on_char_boundary() {
        let long = "à".repeat(200);
        let s = sanitize_component(&long);
        assert!(s.len() <= 90);
        assert!(!s.is_empty());
    }

    #[test]
    fn file_name_composes() {
        assert_eq!(
            doc_file_name(Some("Titolo bello"), Some(2023), Some("Rossi"), 7),
            "Rossi 2023 — Titolo bello.pdf"
        );
        assert_eq!(doc_file_name(None, None, None, 7), "Documento 7.pdf");
    }
}
