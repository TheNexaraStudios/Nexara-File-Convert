mod commands;
mod conversion;
pub mod provisioning;
mod tools;

use conversion::engine;
use conversion::jobs::JobRegistry;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Clean up any temp working files left behind by a previous run that
    // crashed or was force-closed mid-conversion.
    let _ = std::fs::remove_dir_all(std::env::temp_dir().join("NexaraFileConvert"));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(JobRegistry::default())
        .setup(|app| {
            // Resolve every engine's real binary path once at startup —
            // bundled/downloaded app-data location first, falling back to
            // the existing Program-Files/PATH probing — so conversions
            // never depend on system PATH. Re-run after first-run
            // provisioning completes (see `run_engine_provisioning`) to
            // pick up anything newly extracted/installed without a
            // restart.
            engine::init_resolved_binaries(&app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_format_registry,
            commands::get_engine_status,
            commands::probe_media,
            commands::get_file_meta,
            commands::start_conversion,
            commands::cancel_conversion,
            commands::tools::get_pdf_page_count,
            commands::tools::merge_pdfs,
            commands::tools::split_pdf,
            commands::tools::export_pdf_pages,
            commands::tools::preview_archive,
            commands::tools::extract_archive,
            commands::tools::create_archive,
            commands::tools::inspect_metadata,
            commands::provisioning::get_engine_readiness,
            commands::provisioning::run_engine_provisioning,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
