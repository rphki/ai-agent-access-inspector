//! Which paths the frontend may open. The frontend is not trusted: a path is
//! only opened if the last scan reported it, and only configuration files
//! with a known text format are handed to their default application.

use std::fs;
use std::path::{Path, PathBuf};

use inspector_core::model::ScanReport;

use super::{CommandError, ErrorKind};

/// File extensions that are opened with their default application.
const OPENABLE_EXTENSIONS: &[&str] = &["json", "md"];

#[derive(Debug, PartialEq, Eq)]
pub enum OpenAction {
    /// Open the (symlink-resolved) file with its default application.
    Open(PathBuf),
    /// Only show the path in the file manager.
    Reveal(PathBuf),
}

/// Paths that may be shown in the file manager: everything the report
/// names as a location.
pub fn revealable_paths(report: &ScanReport) -> Vec<&Path> {
    let mut paths = source_files(report);
    for result in &report.results {
        paths.extend(result.detection.evidence.iter().map(PathBuf::as_path));
        for finding in &result.findings {
            paths.extend(finding.target.base_path.as_deref());
            paths.extend(finding.path_state.symlink_target.as_deref());
        }
    }
    paths
}

/// Configuration files named as the source of a finding or diagnostic.
pub fn source_files(report: &ScanReport) -> Vec<&Path> {
    let mut paths = Vec::new();
    for result in &report.results {
        for finding in &result.findings {
            paths.extend(finding.source.file.as_deref());
            paths.extend(
                finding
                    .additional_sources
                    .iter()
                    .filter_map(|s| s.file.as_deref()),
            );
        }
        paths.extend(result.diagnostics.iter().filter_map(|d| d.file.as_deref()));
    }
    paths
}

pub fn check_reveal(allowed: &[&Path], requested: &Path) -> Result<PathBuf, CommandError> {
    if allowed.contains(&requested) {
        Ok(requested.to_path_buf())
    } else {
        Err(not_in_report(requested))
    }
}

pub fn check_open(allowed: &[&Path], requested: &Path) -> Result<OpenAction, CommandError> {
    if !allowed.contains(&requested) {
        return Err(not_in_report(requested));
    }
    let resolved = fs::canonicalize(requested).map_err(|e| {
        CommandError::new(
            ErrorKind::OpenFailed,
            format!("{}: {e}", requested.display()),
        )
    })?;
    let is_file = fs::metadata(&resolved).is_ok_and(|m| m.is_file());
    let known_format = resolved
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| OPENABLE_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()));
    if is_file && known_format {
        Ok(OpenAction::Open(resolved))
    } else {
        Ok(OpenAction::Reveal(requested.to_path_buf()))
    }
}

fn not_in_report(path: &Path) -> CommandError {
    CommandError::new(ErrorKind::PathNotInReport, path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use inspector_core::model::{ScanResult, ToolInfo};
    use inspector_core::platform::OperatingSystem;

    fn fixture_report() -> ScanReport {
        let json = include_str!("../../../fixtures/claude-code/linux-basic/expected.json");
        let result: ScanResult = serde_json::from_str(json).unwrap();
        ScanReport {
            format_version: "test".into(),
            tool: ToolInfo::current(),
            operating_system: OperatingSystem::Linux,
            results: vec![result],
        }
    }

    /// A fresh directory with a few files, removed when dropped.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("inspector-gui-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("folder")).unwrap();
            for file in ["settings.json", "CLAUDE.md", "script.sh", "UPPER.JSON"] {
                fs::write(dir.join(file), "{}").unwrap();
            }
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn report_paths_include_sources_and_targets() {
        let report = fixture_report();
        let sources = source_files(&report);
        assert!(sources.contains(&Path::new("/home/alice/.claude.json")));
        let revealable = revealable_paths(&report);
        assert!(revealable.contains(&Path::new("/home/alice/app")));
        assert!(revealable.contains(&Path::new("/home/alice/.claude")));
        assert!(!sources.contains(&Path::new("/home/alice/app")));
    }

    #[test]
    fn reveal_only_reported_paths() {
        let report = fixture_report();
        let allowed = revealable_paths(&report);
        assert!(check_reveal(&allowed, Path::new("/home/alice/app")).is_ok());
        let err = check_reveal(&allowed, Path::new("/bin/sh")).unwrap_err();
        assert_eq!(err.kind, ErrorKind::PathNotInReport);
        // No normalisation: a path that only resolves to a reported one is rejected.
        assert!(check_reveal(&allowed, Path::new("/home/alice/app/../app")).is_err());
    }

    #[test]
    fn open_rejects_unreported_paths_before_touching_the_disk() {
        let dir = TempDir::new("unreported");
        let file = dir.0.join("settings.json");
        let err = check_open(&[], &file).unwrap_err();
        assert_eq!(err.kind, ErrorKind::PathNotInReport);
    }

    #[test]
    fn open_only_json_and_markdown_files() {
        let dir = TempDir::new("formats");
        let paths: Vec<PathBuf> = [
            "settings.json",
            "CLAUDE.md",
            "UPPER.JSON",
            "script.sh",
            "folder",
        ]
        .iter()
        .map(|name| dir.0.join(name))
        .collect();
        let allowed: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
        let action = |i: usize| check_open(&allowed, &paths[i]).unwrap();

        assert!(matches!(action(0), OpenAction::Open(_)));
        assert!(matches!(action(1), OpenAction::Open(_)));
        assert!(matches!(action(2), OpenAction::Open(_)));
        assert_eq!(action(3), OpenAction::Reveal(paths[3].clone()));
        assert_eq!(action(4), OpenAction::Reveal(paths[4].clone()));
    }

    #[test]
    fn open_missing_file_fails() {
        let missing = std::env::temp_dir().join("inspector-gui-does-not-exist.json");
        let err = check_open(&[missing.as_path()], &missing).unwrap_err();
        assert_eq!(err.kind, ErrorKind::OpenFailed);
    }

    #[cfg(unix)]
    #[test]
    fn open_follows_symlinks_and_checks_the_target() {
        let dir = TempDir::new("symlink");
        let to_script = dir.0.join("innocent.json");
        let to_settings = dir.0.join("link.md");
        std::os::unix::fs::symlink(dir.0.join("script.sh"), &to_script).unwrap();
        std::os::unix::fs::symlink(dir.0.join("settings.json"), &to_settings).unwrap();
        let allowed = [to_script.as_path(), to_settings.as_path()];

        assert_eq!(
            check_open(&allowed, &to_script).unwrap(),
            OpenAction::Reveal(to_script.clone())
        );
        assert_eq!(
            check_open(&allowed, &to_settings).unwrap(),
            OpenAction::Open(fs::canonicalize(dir.0.join("settings.json")).unwrap())
        );
    }
}
