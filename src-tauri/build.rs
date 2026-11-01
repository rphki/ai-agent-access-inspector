/// Commands of the application. Declaring them here makes Tauri generate a
/// permission per command, so a window may only call the commands its
/// capability (`capabilities/default.json`) lists.
const COMMANDS: &[&str] = &[
    "list_scanners",
    "run_scan",
    "get_scan_details",
    "get_app_info",
    "open_path_in_file_manager",
    "open_source_file",
    "export_scan",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("failed to run the Tauri build script");
}
