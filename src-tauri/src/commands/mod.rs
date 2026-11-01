//! Tauri commands, the only interface between frontend and core.

mod export;
mod open;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use inspector_core::context::ScanContext;
use inspector_core::export::anonymize::PathPrivacy;
use inspector_core::model::{ScanReport, ScannerMetadata};
use inspector_core::platform::OperatingSystem;
use inspector_core::scanner;
use inspector_core::{APP_NAME, VERSION};
use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use export::ExportFormat;
use open::OpenAction;

/// The result of the last scan. Paths may only be opened if this report
/// contains them.
#[derive(Default)]
pub struct AppState {
    last_scan: Mutex<Option<StoredScan>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredScan {
    pub report: ScanReport,
    /// End of the scan in milliseconds since the Unix epoch.
    pub finished_at: u64,
    /// Home directory of the scan, for anonymised exports. Not sent to the
    /// frontend.
    #[serde(skip)]
    pub home: String,
}

/// Error returned to the frontend. The frontend translates `kind`; `detail`
/// is a technical value (path, operating system message) and is not
/// translated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub kind: ErrorKind,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ErrorKind {
    /// The scan context (home directory, environment) could not be set up.
    ScanContext,
    /// The scan thread failed unexpectedly.
    ScanFailed,
    /// No scan has run yet.
    NoScan,
    /// The path does not appear in the last scan.
    PathNotInReport,
    /// The operating system could not open the path.
    OpenFailed,
    /// The export file could not be written.
    ExportFailed,
}

impl CommandError {
    fn new(kind: ErrorKind, detail: impl Into<Option<String>>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerInfo {
    #[serde(flatten)]
    pub metadata: ScannerMetadata,
    /// Runs without being selected explicitly.
    pub default: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub operating_system: OperatingSystem,
}

#[tauri::command]
pub fn list_scanners() -> Vec<ScannerInfo> {
    let defaults: Vec<String> = scanner::default_scanners()
        .iter()
        .map(|s| s.metadata().id)
        .collect();
    scanner::all_scanners()
        .iter()
        .map(|s| {
            let metadata = s.metadata();
            ScannerInfo {
                default: defaults.contains(&metadata.id),
                metadata,
            }
        })
        .collect()
}

/// Scans the real system with all default scanners.
#[tauri::command]
pub async fn run_scan(state: State<'_, AppState>) -> Result<StoredScan, CommandError> {
    let (report, home) = tauri::async_runtime::spawn_blocking(|| {
        let context = ScanContext::from_system()
            .map_err(|e| CommandError::new(ErrorKind::ScanContext, e.to_string()))?;
        let report = scanner::run_scan(&context, &scanner::default_scanners());
        Ok::<_, CommandError>((report, context.home_dir.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| CommandError::new(ErrorKind::ScanFailed, e.to_string()))??;

    let finished_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default();
    let stored = StoredScan {
        report,
        finished_at,
        home,
    };
    *lock(&state) = Some(stored.clone());
    Ok(stored)
}

/// The last scan, e.g. after the frontend was reloaded.
#[tauri::command]
pub fn get_scan_details(state: State<'_, AppState>) -> Option<StoredScan> {
    lock(&state).clone()
}

#[tauri::command]
pub fn get_app_info() -> AppInfo {
    AppInfo {
        name: APP_NAME,
        version: VERSION,
        operating_system: OperatingSystem::current(),
    }
}

/// Shows a reported path in the file manager. Nothing is opened or run.
#[tauri::command]
pub fn open_path_in_file_manager(
    path: PathBuf,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let path = {
        let guard = lock(&state);
        let report = &guard
            .as_ref()
            .ok_or(CommandError::new(ErrorKind::NoScan, None))?
            .report;
        open::check_reveal(&open::revealable_paths(report), &path)?
    };
    reveal(&path)
}

/// Opens a reported configuration file with its default application, or
/// shows it in the file manager if it is not a plain JSON or Markdown file.
#[tauri::command]
pub fn open_source_file(
    path: PathBuf,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let action = {
        let guard = lock(&state);
        let report = &guard
            .as_ref()
            .ok_or(CommandError::new(ErrorKind::NoScan, None))?
            .report;
        open::check_open(&open::source_files(report), &path)?
    };
    match action {
        OpenAction::Open(path) => app
            .opener()
            .open_path(path.to_string_lossy(), None::<&str>)
            .map_err(|e| CommandError::new(ErrorKind::OpenFailed, e.to_string())),
        OpenAction::Reveal(path) => reveal(&path),
    }
}

/// Exports the last scan. The file is chosen in a save dialog opened here,
/// never by the frontend. `translations` holds the texts of the GUI language
/// for Markdown; JSON always carries keys and English texts. Returns the
/// written path, or `None` if the dialog was cancelled.
#[tauri::command]
pub async fn export_scan(
    format: ExportFormat,
    privacy: PathPrivacy,
    translations: Option<HashMap<String, String>>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, CommandError> {
    let stored = lock(&state)
        .clone()
        .ok_or(CommandError::new(ErrorKind::NoScan, None))?;
    let content = export::render(&stored, format, privacy, translations.as_ref());

    let Some(chosen) = app
        .dialog()
        .file()
        .set_file_name(export::file_name(stored.finished_at, format))
        .add_filter(format.label(), &[format.extension()])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = chosen
        .into_path()
        .map_err(|e| CommandError::new(ErrorKind::ExportFailed, e.to_string()))?;
    std::fs::write(&path, content).map_err(|e| {
        CommandError::new(ErrorKind::ExportFailed, format!("{}: {e}", path.display()))
    })?;
    Ok(Some(path.display().to_string()))
}

fn reveal(path: &std::path::Path) -> Result<(), CommandError> {
    tauri_plugin_opener::reveal_item_in_dir(path)
        .map_err(|e| CommandError::new(ErrorKind::OpenFailed, e.to_string()))
}

/// A poisoned lock only means an earlier command panicked; the stored scan
/// is still valid.
fn lock<'a>(state: &'a State<'_, AppState>) -> std::sync::MutexGuard<'a, Option<StoredScan>> {
    state
        .last_scan
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
