//! Desktop application of the AI Agent Access Inspector.
//!
//! A thin layer over `inspector-core`: the commands in [`commands`] run scans
//! and open paths that a scan reported. Nothing here changes agent
//! configuration.

mod commands;

use commands::AppState;

/// Starts the application.
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.menu(macos_menu);
    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_scanners,
            commands::run_scan,
            commands::get_scan_details,
            commands::get_app_info,
            commands::open_path_in_file_manager,
            commands::open_source_file,
            commands::export_scan,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the application");
}

/// The default macOS menu with our own About panel: copyright on two lines
/// and no build number, which would repeat the version as "0.4.0 (0.4.0)".
#[cfg(target_os = "macos")]
fn macos_menu<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{AboutMetadata, Menu, PredefinedMenuItem};

    let menu = Menu::default(handle)?;
    let Some(app_menu) = menu.items()?.into_iter().next() else {
        return Ok(menu);
    };
    let Some(app_menu) = app_menu.as_submenu() else {
        return Ok(menu);
    };
    let package = handle.package_info();
    let metadata = AboutMetadata {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        // An empty string hides the build number (CFBundleVersion).
        short_version: Some(String::new()),
        copyright: Some("Copyright 2026 Raphael Kirchner\nApache-2.0".into()),
        ..Default::default()
    };
    app_menu.remove_at(0)?;
    app_menu.insert(&PredefinedMenuItem::about(handle, None, Some(metadata))?, 0)?;
    Ok(menu)
}
