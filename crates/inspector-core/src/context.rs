//! The scan context: everything a scanner may know about the system.
//!
//! Scanners never read the real home directory or environment directly.
//! They go through a [`ScanContext`], which is either taken from the running
//! system or loaded from a fixture directory, so that every platform can be
//! tested on every development machine.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::platform::OperatingSystem;

/// Environment variables captured from the system. Only these are visible to
/// scanners, so that unrelated secrets in the environment are never read.
pub const CAPTURED_ENV_VARS: &[&str] = &[
    "APPDATA",
    "CLAUDE_CODE_PROJECT_DIR_NAME",
    "CLAUDE_CONFIG_DIR",
    "HOME",
    "LOCALAPPDATA",
    "ProgramData",
    "ProgramFiles",
    "TMPDIR",
    "USERPROFILE",
    "XDG_CONFIG_HOME",
];

/// Name of the manifest file inside a fixture directory.
pub const FIXTURE_MANIFEST: &str = "fixture.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanContext {
    pub home_dir: PathBuf,
    /// Directory to treat as the current project, if any.
    pub current_dir: Option<PathBuf>,
    pub operating_system: OperatingSystem,
    pub env: EnvSnapshot,
    /// In fixture scans, the directory that stands in for the filesystem
    /// root. `None` for system scans.
    pub system_root: Option<PathBuf>,
}

impl ScanContext {
    /// Context of the running system.
    pub fn from_system() -> Result<Self, ContextError> {
        let home_dir = std::env::home_dir().ok_or(ContextError::NoHomeDirectory)?;
        Ok(Self {
            home_dir,
            current_dir: None,
            operating_system: OperatingSystem::current(),
            env: EnvSnapshot::from_system(),
            system_root: None,
        })
    }

    /// Context described by `fixture.json` in `root`.
    ///
    /// System-wide paths are looked up below `root/system`, the home
    /// directory and current directory are relative to `root`.
    pub fn from_fixture(root: &Path) -> Result<Self, ContextError> {
        let manifest_path = root.join(FIXTURE_MANIFEST);
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| ContextError::Fixture(format!("{}: {e}", manifest_path.display())))?;
        let manifest: FixtureManifest = serde_json::from_str(&text)
            .map_err(|e| ContextError::Fixture(format!("{}: {e}", manifest_path.display())))?;
        Ok(Self {
            home_dir: root.join(manifest.home),
            current_dir: manifest.current_dir.map(|dir| root.join(dir)),
            operating_system: manifest.operating_system,
            env: EnvSnapshot(manifest.env),
            system_root: Some(root.join("system")),
        })
    }

    /// Maps an absolute system path such as `/etc/claude-code` or
    /// `C:\Program Files\ClaudeCode` to the path to read.
    ///
    /// In system scans the path is returned unchanged. In fixture scans it is
    /// placed below the fixture's system root; a drive letter becomes a
    /// directory (`C:` → `C`), so Windows fixtures work on every platform.
    pub fn system_path(&self, path: &str) -> PathBuf {
        let Some(root) = &self.system_root else {
            return PathBuf::from(path);
        };
        let mut mapped = root.clone();
        for component in path.split(['/', '\\']).filter(|c| !c.is_empty()) {
            mapped.push(component.strip_suffix(':').unwrap_or(component));
        }
        mapped
    }

    pub fn env_var(&self, name: &str) -> Option<&str> {
        self.env.get(name)
    }
}

/// Selected environment variables, see [`CAPTURED_ENV_VARS`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvSnapshot(BTreeMap<String, String>);

impl EnvSnapshot {
    pub fn from_system() -> Self {
        Self(
            CAPTURED_ENV_VARS
                .iter()
                .filter_map(|name| Some((name.to_string(), std::env::var(name).ok()?)))
                .collect(),
        )
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn set(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.0.insert(name.into(), value.into());
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FixtureManifest {
    operating_system: OperatingSystem,
    #[serde(default = "default_fixture_home")]
    home: PathBuf,
    current_dir: Option<PathBuf>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

fn default_fixture_home() -> PathBuf {
    PathBuf::from("home")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextError {
    NoHomeDirectory,
    Fixture(String),
}

impl fmt::Display for ContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoHomeDirectory => write!(f, "home directory could not be determined"),
            Self::Fixture(message) => write!(f, "invalid fixture: {message}"),
        }
    }
}

impl std::error::Error for ContextError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_context(root: &Path) -> ScanContext {
        ScanContext {
            home_dir: root.join("home"),
            current_dir: None,
            operating_system: OperatingSystem::Windows,
            env: EnvSnapshot::default(),
            system_root: Some(root.join("system")),
        }
    }

    #[test]
    fn system_path_is_unchanged_in_system_scans() {
        let mut context = fixture_context(Path::new("/fx"));
        context.system_root = None;
        assert_eq!(
            context.system_path("/etc/claude-code"),
            PathBuf::from("/etc/claude-code")
        );
    }

    #[test]
    fn system_path_maps_unix_paths_below_fixture_root() {
        let context = fixture_context(Path::new("fx"));
        assert_eq!(
            context.system_path("/etc/claude-code/managed-settings.json"),
            Path::new("fx/system/etc/claude-code/managed-settings.json")
        );
    }

    #[test]
    fn system_path_maps_windows_drive_paths() {
        let context = fixture_context(Path::new("fx"));
        assert_eq!(
            context.system_path(r"C:\Program Files\ClaudeCode"),
            Path::new("fx/system/C/Program Files/ClaudeCode")
        );
    }

    #[test]
    fn loads_fixture_manifest() {
        let root = std::env::temp_dir().join(format!("inspector-fixture-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(FIXTURE_MANIFEST),
            r#"{"operatingSystem":"linux","home":"home/alice","currentDir":"home/alice/projekt","env":{"CLAUDE_CONFIG_DIR":"/srv/claude"}}"#,
        )
        .unwrap();
        let context = ScanContext::from_fixture(&root).unwrap();
        std::fs::remove_dir_all(&root).unwrap();

        assert_eq!(context.operating_system, OperatingSystem::Linux);
        assert_eq!(context.home_dir, root.join("home/alice"));
        assert_eq!(context.current_dir, Some(root.join("home/alice/projekt")));
        assert_eq!(context.env_var("CLAUDE_CONFIG_DIR"), Some("/srv/claude"));
        assert_eq!(context.system_root, Some(root.join("system")));
    }

    #[test]
    fn missing_fixture_manifest_is_an_error() {
        let error = ScanContext::from_fixture(Path::new("does/not/exist")).unwrap_err();
        assert!(matches!(error, ContextError::Fixture(_)));
    }

    #[test]
    fn system_env_snapshot_only_contains_captured_variables() {
        let snapshot = EnvSnapshot::from_system();
        for name in snapshot.0.keys() {
            assert!(CAPTURED_ENV_VARS.contains(&name.as_str()));
        }
    }
}
