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

use crate::analysis::path::TargetPath;
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

/// All paths in the context are paths of the scanned system ("target
/// paths"). Files are read through [`ScanContext::host_path`], which maps a
/// target path to the location on the machine running the inspector.
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
    /// The manifest's `home` and `currentDir` are absolute paths of the
    /// simulated system. Every file of that system lives below
    /// `root/system`, see [`ScanContext::host_path`].
    pub fn from_fixture(root: &Path) -> Result<Self, ContextError> {
        let manifest_path = root.join(FIXTURE_MANIFEST);
        let fixture_error = |message: String| {
            ContextError::Fixture(format!("{}: {message}", manifest_path.display()))
        };
        let text =
            std::fs::read_to_string(&manifest_path).map_err(|e| fixture_error(e.to_string()))?;
        let manifest: FixtureManifest =
            serde_json::from_str(&text).map_err(|e| fixture_error(e.to_string()))?;
        let os = manifest.operating_system;
        for path in std::iter::once(&manifest.home).chain(&manifest.current_dir) {
            match TargetPath::parse(os, path) {
                Ok(parsed) if parsed.is_absolute() => {}
                Ok(_) => return Err(fixture_error(format!("'{path}' is not absolute"))),
                Err(e) => return Err(fixture_error(e.to_string())),
            }
        }
        Ok(Self {
            home_dir: PathBuf::from(manifest.home),
            current_dir: manifest.current_dir.map(PathBuf::from),
            operating_system: os,
            env: EnvSnapshot(manifest.env),
            system_root: Some(root.join("system")),
        })
    }

    /// The home directory as a target path.
    pub fn home(&self) -> Option<TargetPath> {
        TargetPath::parse(self.operating_system, &self.home_dir.to_string_lossy())
            .ok()
            .filter(TargetPath::is_absolute)
    }

    /// Maps an absolute target path such as `/etc/claude-code` or
    /// `C:\Program Files\ClaudeCode` to the host path to read.
    ///
    /// In system scans the path is returned unchanged. In fixture scans it is
    /// placed below the fixture's system root; a drive letter becomes a
    /// directory (`C:` → `C`), so Windows fixtures work on every platform.
    pub fn host_path(&self, target: &str) -> PathBuf {
        let Some(root) = &self.system_root else {
            return PathBuf::from(target);
        };
        let mut mapped = root.clone();
        for component in target.split(['/', '\\']).filter(|c| !c.is_empty()) {
            mapped.push(component.strip_suffix(':').unwrap_or(component));
        }
        mapped
    }

    /// Inverse of [`ScanContext::host_path`]: the target path for a host
    /// path, e.g. a resolved symlink target. `None` if a fixture's host path
    /// lies outside its system root.
    pub fn target_path(&self, host: &Path) -> Option<String> {
        let Some(root) = &self.system_root else {
            return Some(host.to_string_lossy().into_owned());
        };
        // Canonical host paths may differ from `root` by symlinks such as
        // macOS's /var → /private/var, so compare canonical forms.
        let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        let relative = host.strip_prefix(&root).ok()?;
        let components: Vec<String> = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        Some(match self.operating_system {
            OperatingSystem::Windows => match components.split_first() {
                Some((drive, rest)) if drive.len() == 1 => {
                    format!("{drive}:\\{}", rest.join("\\"))
                }
                _ => format!("\\\\{}", components.join("\\")),
            },
            _ => format!("/{}", components.join("/")),
        })
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
    home: String,
    current_dir: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
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
    use crate::test_support::TempDir;

    fn fixture_context(root: &Path, os: OperatingSystem) -> ScanContext {
        ScanContext {
            home_dir: PathBuf::from("/home/alice"),
            current_dir: None,
            operating_system: os,
            env: EnvSnapshot::default(),
            system_root: Some(root.join("system")),
        }
    }

    fn write_manifest(root: &Path, json: &str) {
        std::fs::write(root.join(FIXTURE_MANIFEST), json).unwrap();
    }

    #[test]
    fn host_path_is_unchanged_in_system_scans() {
        let mut context = fixture_context(Path::new("/fx"), OperatingSystem::Linux);
        context.system_root = None;
        assert_eq!(
            context.host_path("/etc/claude-code"),
            PathBuf::from("/etc/claude-code")
        );
    }

    #[test]
    fn host_path_maps_unix_paths_below_fixture_root() {
        let context = fixture_context(Path::new("fx"), OperatingSystem::Linux);
        assert_eq!(
            context.host_path("/etc/claude-code/managed-settings.json"),
            Path::new("fx/system/etc/claude-code/managed-settings.json")
        );
    }

    #[test]
    fn host_path_maps_windows_drive_paths() {
        let context = fixture_context(Path::new("fx"), OperatingSystem::Windows);
        assert_eq!(
            context.host_path(r"C:\Program Files\ClaudeCode"),
            Path::new("fx/system/C/Program Files/ClaudeCode")
        );
    }

    #[test]
    fn target_path_inverts_host_path() {
        let dir = TempDir::new("context-inverse");
        let linux = fixture_context(dir.path(), OperatingSystem::Linux);
        std::fs::create_dir_all(linux.host_path("/srv/data")).unwrap();
        let host = std::fs::canonicalize(linux.host_path("/srv/data")).unwrap();
        assert_eq!(linux.target_path(&host).as_deref(), Some("/srv/data"));

        let windows = fixture_context(dir.path(), OperatingSystem::Windows);
        std::fs::create_dir_all(windows.host_path(r"C:\Users\alice")).unwrap();
        let host = std::fs::canonicalize(windows.host_path(r"C:\Users\alice")).unwrap();
        assert_eq!(
            windows.target_path(&host).as_deref(),
            Some(r"C:\Users\alice")
        );
    }

    #[test]
    fn loads_fixture_manifest() {
        let dir = TempDir::new("context-manifest");
        write_manifest(
            dir.path(),
            r#"{"operatingSystem":"linux","home":"/home/alice","currentDir":"/home/alice/project","env":{"CLAUDE_CONFIG_DIR":"/srv/claude"}}"#,
        );
        let context = ScanContext::from_fixture(dir.path()).unwrap();

        assert_eq!(context.operating_system, OperatingSystem::Linux);
        assert_eq!(context.home_dir, PathBuf::from("/home/alice"));
        assert_eq!(context.home().unwrap().to_string(), "/home/alice");
        assert_eq!(
            context.current_dir,
            Some(PathBuf::from("/home/alice/project"))
        );
        assert_eq!(context.env_var("CLAUDE_CONFIG_DIR"), Some("/srv/claude"));
        assert_eq!(context.system_root, Some(dir.path().join("system")));
    }

    #[test]
    fn fixture_paths_must_be_absolute() {
        let dir = TempDir::new("context-relative");
        write_manifest(
            dir.path(),
            r#"{"operatingSystem":"windows","home":"Users\\alice"}"#,
        );
        let error = ScanContext::from_fixture(dir.path()).unwrap_err();
        assert!(error.to_string().contains("not absolute"), "{error}");
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
