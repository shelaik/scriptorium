mod ai;
mod bibtex;
mod citation;
mod commands;
mod connector;
mod db;
mod discovery;
mod embed;
mod github;
mod import;
mod metadata;
mod model;
mod obsidian;
mod ocr;
mod rag;
mod secret;
mod table;
mod pdf;
mod term;
mod watch;
mod wiki;

use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use tauri::Manager;

/// Shared application state, managed by Tauri and accessible from commands.
pub struct AppState {
    /// The single SQLite connection (serialized behind a mutex).
    pub db: Mutex<rusqlite::Connection>,
    /// A single pdfium instance (internally thread-safe via the `thread_safe` feature).
    pub pdfium: pdfium_render::prelude::Pdfium,
    /// Serializes *whole* pdfium document operations. `thread_safe` only locks
    /// individual FFI calls, so two threads processing different documents can
    /// still interleave and corrupt pdfium's global state (native 0xc0000409
    /// crash) — e.g. the startup watched-folder scan overlapping a browser-grabbed
    /// import. Hold this across a full extract/render to keep them one-at-a-time.
    pub pdfium_lock: parking_lot::Mutex<()>,
    /// Set to request cancellation of an in-progress embedding job.
    pub cancel_embed: AtomicBool,
    /// Set to request cancellation of an in-progress RAG indexing job.
    pub rag_cancel: AtomicBool,
    /// Set to request cancellation of an in-progress wiki generation.
    pub wiki_cancel: AtomicBool,
    /// The active watched-folder watcher (dropping it stops watching).
    pub watcher: Mutex<Option<notify::RecommendedWatcher>>,
    /// The running browser-connector loopback server, if any (drop = stop).
    pub connector: Mutex<Option<connector::ConnectorHandle>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Must happen before any SQLite connection is opened.
    db::register_sqlite_vec();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("pdfmanage.db");
            let conn = db::open(&db_path).map_err(|e| e.to_string())?;
            let pdfium = pdf::bind_for_app(app.handle()).map_err(|e| e.to_string())?;
            app.manage(AppState {
                db: Mutex::new(conn),
                pdfium,
                pdfium_lock: Mutex::new(()),
                cancel_embed: AtomicBool::new(false),
                rag_cancel: AtomicBool::new(false),
                wiki_cancel: AtomicBool::new(false),
                watcher: Mutex::new(None),
                connector: Mutex::new(None),
            });
            app.manage(term::TermState::default());
            // One-time migration: move any plaintext API keys from the settings
            // table into the OS credential vault, then blank them in the DB.
            {
                let state = app.state::<AppState>();
                let conn = state.db.lock();
                commands::migrate_keys_to_vault(&conn);
            }
            // Resume watching a previously-chosen folder, if any.
            let watched: Option<String> = {
                let state = app.state::<AppState>();
                let conn = state.db.lock();
                conn.query_row(
                    "SELECT value FROM settings WHERE key = 'watched_folder'",
                    [],
                    |r| r.get::<_, String>(0),
                )
                .ok()
            };
            if let Some(dir) = watched {
                if let Ok(w) = watch::start(app.handle().clone(), &dir) {
                    app.state::<AppState>().watcher.lock().replace(w);
                }
                // Catch up on PDFs added while the app was closed.
                watch::scan_existing(app.handle().clone(), dir);
            }
            // Start the browser connector (loopback bookmarklet endpoint) unless
            // the user disabled it. Non-fatal if the port can't be bound.
            commands::start_connector(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_files,
            commands::list_documents,
            commands::get_thumbnail,
            commands::rebuild_thumbnails,
            commands::read_pdf,
            commands::enrich_all,
            commands::repair_metadata,
            commands::embedding_status,
            commands::generate_embeddings,
            commands::cancel_embeddings,
            commands::search,
            commands::related_documents,
            commands::similarity_graph,
            commands::rag_index_status,
            commands::build_rag_index,
            commands::cancel_rag_index,
            commands::clear_rag_index,
            commands::ask_library,
            commands::cite_text,
            commands::export_citations,
            commands::citation_links,
            commands::citation_gaps,
            commands::library_health,
            commands::library_facets,
            commands::list_saved_searches,
            commands::create_saved_search,
            commands::delete_saved_search,
            commands::run_saved_search,
            commands::get_obsidian_vault,
            commands::set_obsidian_vault,
            commands::export_obsidian,
            commands::list_annotations,
            commands::add_annotation,
            commands::update_annotation_note,
            commands::delete_annotation,
            commands::set_document_notes,
            commands::list_tags,
            commands::create_tag,
            commands::update_tag,
            commands::delete_tag,
            commands::set_document_tags,
            commands::list_collections,
            commands::create_collection,
            commands::delete_collection,
            commands::add_to_collection,
            commands::remove_from_collection,
            commands::get_watched_folder,
            commands::set_watched_folder,
            commands::get_document_meta,
            commands::update_document_metadata,
            commands::set_read,
            commands::set_favorite,
            commands::set_last_page,
            commands::get_last_page,
            commands::recent_documents,
            commands::documents_by_author,
            commands::backup_library,
            commands::delete_documents,
            commands::restore_documents,
            commands::purge_documents,
            commands::list_trash,
            commands::add_document_tag,
            commands::find_duplicates,
            commands::merge_documents,
            commands::ocr_document,
            commands::add_by_identifiers,
            commands::add_from_url,
            commands::get_connector_info,
            commands::set_connector_enabled,
            commands::import_bibtex,
            commands::find_pdf,
            commands::attach_from_url,
            commands::hf_resources,
            commands::github_repos,
            commands::github_readme,
            commands::extract_table,
            commands::export_table,
            commands::ai_clean_table,
            commands::extract_region_text,
            commands::write_text_file,
            commands::get_discovery_settings,
            commands::set_discovery_settings,
            commands::set_api_key,
            commands::discover_search,
            commands::discover_add,
            commands::explore_citations,
            commands::get_ai_settings,
            commands::set_ai_settings,
            commands::ai_list_models,
            commands::ai_status,
            commands::summarize_document,
            commands::ai_explain,
            commands::autotag_document,
            commands::wiki_list,
            commands::wiki_get,
            commands::wiki_generate,
            commands::wiki_delete,
            commands::wiki_cancel,
            commands::compare_documents,
            commands::generate_review,
            commands::harvest_results,
            commands::reading_path,
            commands::document_path,
            commands::copy_pdfs_to_clipboard,
            commands::open_external,
            commands::reveal_pdf,
            commands::share_via_outlook,
            commands::ai_server_start,
            commands::ai_server_stop,
            commands::term_open,
            commands::term_write,
            commands::term_resize,
            commands::term_close,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // Kill the terminal's shell child on exit so it isn't orphaned, and
            // free the connector's loopback socket.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                term::close(app_handle.state::<term::TermState>().inner());
                commands::stop_connector(app_handle);
            }
        });
}
