//! Desktop application of the AI Agent Access Inspector.
//!
//! A thin layer over `inspector-core`: the commands in [`commands`] run scans
//! and open paths that a scan reported. Nothing here changes agent
//! configuration.

mod commands;

use commands::AppState;

/// Starts the application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_scanners,
            commands::run_scan,
            commands::get_scan_details,
            commands::get_app_info,
            commands::open_path_in_file_manager,
            commands::open_source_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
