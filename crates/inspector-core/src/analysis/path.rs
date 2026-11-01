//! Paths of the scanned system ("target paths").
//!
//! The scanned system is not necessarily the one the inspector runs on: a
//! fixture for Windows is tested on Linux and vice versa. `std::path` follows
//! the rules of the host, so target paths get their own type that follows
//! the rules of the target operating system.

use std::fmt;
use std::path::PathBuf;

use crate::platform::OperatingSystem;

/// A lexically normalised path on the scanned system.
///
/// `.` components are removed, `..` components are resolved where possible
/// (never above the root) and separators are unified. Drive letters are
/// stored in upper case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetPath {
    os: OperatingSystem,
    root: Root,
    components: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Root {
    Relative,
    /// `/` on macOS and Linux.
    Unix,
    /// `C:\` on Windows.
    Drive(char),
    /// `\\server\share` on Windows.
    Unc {
        server: String,
        share: String,
    },
}

impl TargetPath {
    pub fn parse(os: OperatingSystem, input: &str) -> Result<Self, PathError> {
        if input.is_empty() {
            return Err(PathError::Empty);
        }
        let (root, rest) = match os {
            OperatingSystem::Windows => parse_windows_root(input)?,
            OperatingSystem::MacOs | OperatingSystem::Linux => match input.strip_prefix('/') {
                Some(rest) => (Root::Unix, rest.to_string()),
                None => (Root::Relative, input.to_string()),
            },
        };
        let mut path = Self {
            os,
            root,
            components: Vec::new(),
        };
        let separators: &[char] = match os {
            OperatingSystem::Windows => &['\\'],
            _ => &['/'],
        };
        for component in rest.split(separators) {
            check_component(os, input, component)?;
            path.push_component(component);
        }
        Ok(path)
    }

    pub fn os(&self) -> OperatingSystem {
        self.os
    }

    pub fn is_absolute(&self) -> bool {
        self.root != Root::Relative
    }

    /// The root of the filesystem (`/`) or of a drive or share.
    pub fn is_root(&self) -> bool {
        self.is_absolute() && self.components.is_empty()
    }

    /// A Windows drive root (`C:\`) or share root (`\\server\share`).
    pub fn is_drive_root(&self) -> bool {
        matches!(self.root, Root::Drive(_) | Root::Unc { .. }) && self.components.is_empty()
    }

    pub fn components(&self) -> &[String] {
        &self.components
    }

    /// Appends `relative`. An absolute `relative` replaces the path.
    pub fn join(&self, relative: &str) -> Result<Self, PathError> {
        let other = Self::parse(self.os, relative)?;
        if other.is_absolute() {
            return Ok(other);
        }
        let mut joined = self.clone();
        for component in &other.components {
            joined.push_component(component);
        }
        Ok(joined)
    }

    pub fn parent(&self) -> Option<Self> {
        if self.components.is_empty() || self.components.last().is_some_and(|c| c == "..") {
            return None;
        }
        let mut parent = self.clone();
        parent.components.pop();
        Some(parent)
    }

    /// Whether `self` is `base` or lies below it. Letter case is ignored on
    /// Windows and macOS, whose default filesystems are case-insensitive.
    pub fn starts_with(&self, base: &Self) -> bool {
        self.os == base.os
            && self.root_key() == base.root_key()
            && base.components.len() <= self.components.len()
            && base
                .components
                .iter()
                .zip(&self.components)
                .all(|(a, b)| self.fold(a) == self.fold(b))
    }

    pub fn same_path(&self, other: &Self) -> bool {
        self.components.len() == other.components.len() && self.starts_with(other)
    }

    /// A string that is equal for paths that refer to the same location.
    pub fn comparison_key(&self) -> String {
        self.fold(&self.to_string())
    }

    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.to_string())
    }

    fn push_component(&mut self, component: &str) {
        match component {
            "" | "." => {}
            ".." => match self.components.last() {
                Some(last) if last != ".." => {
                    self.components.pop();
                }
                // `..` above the root stays at the root.
                _ if self.is_absolute() => {}
                _ => self.components.push("..".into()),
            },
            _ => self.components.push(component.into()),
        }
    }

    fn root_key(&self) -> String {
        match &self.root {
            Root::Relative => String::new(),
            Root::Unix => "/".into(),
            Root::Drive(letter) => format!("{letter}:"),
            Root::Unc { server, share } => self.fold(&format!(r"\\{server}\{share}")),
        }
    }

    fn fold(&self, text: &str) -> String {
        if self.os.is_case_insensitive() {
            text.to_lowercase()
        } else {
            text.to_string()
        }
    }
}

impl fmt::Display for TargetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let separator = match self.os {
            OperatingSystem::Windows => "\\",
            _ => "/",
        };
        let joined = self.components.join(separator);
        match &self.root {
            Root::Relative if joined.is_empty() => write!(f, "."),
            Root::Relative => write!(f, "{joined}"),
            Root::Unix => write!(f, "/{joined}"),
            Root::Drive(letter) => write!(f, "{letter}:\\{joined}"),
            Root::Unc { server, share } if joined.is_empty() => write!(f, r"\\{server}\{share}"),
            Root::Unc { server, share } => write!(f, r"\\{server}\{share}\{joined}"),
        }
    }
}

/// Splits a Windows path into its root and the remainder, using `\` as the
/// only separator.
fn parse_windows_root(input: &str) -> Result<(Root, String), PathError> {
    let path = input.replace('/', "\\");
    if let Some(rest) = path.strip_prefix(r"\\?\") {
        // Verbatim paths: `\\?\C:\…` and `\\?\UNC\server\share\…`.
        if let Some(unc) = rest.strip_prefix(r"UNC\") {
            return parse_unc(input, unc);
        }
        return match parse_drive(input, rest)? {
            (Root::Drive(letter), rest) => Ok((Root::Drive(letter), rest)),
            _ => Err(PathError::DevicePath(input.into())),
        };
    }
    if path.starts_with(r"\\.\") {
        return Err(PathError::DevicePath(input.into()));
    }
    if let Some(rest) = path.strip_prefix(r"\\") {
        return parse_unc(input, rest);
    }
    parse_drive(input, &path)
}

fn parse_drive(input: &str, path: &str) -> Result<(Root, String), PathError> {
    let mut chars = path.chars();
    if let (Some(letter), Some(':')) = (chars.next(), chars.next()) {
        if !letter.is_ascii_alphabetic() {
            return Err(PathError::InvalidCharacter {
                path: input.into(),
                character: ':',
            });
        }
        let rest = chars.as_str();
        return match rest.strip_prefix('\\') {
            Some(rest) => Ok((Root::Drive(letter.to_ascii_uppercase()), rest.into())),
            // `C:foo` is relative to the current directory of drive C.
            None => Err(PathError::DriveRelative(input.into())),
        };
    }
    if path.starts_with('\\') {
        // `\foo` is relative to the root of the current drive.
        return Err(PathError::NoDrive(input.into()));
    }
    Ok((Root::Relative, path.into()))
}

fn parse_unc(input: &str, rest: &str) -> Result<(Root, String), PathError> {
    let mut parts = rest.splitn(3, '\\');
    let server = parts.next().unwrap_or_default();
    let share = parts.next().unwrap_or_default();
    if server.is_empty() || share.is_empty() {
        return Err(PathError::InvalidUnc(input.into()));
    }
    Ok((
        Root::Unc {
            server: server.into(),
            share: share.into(),
        },
        parts.next().unwrap_or_default().into(),
    ))
}

fn check_component(os: OperatingSystem, input: &str, component: &str) -> Result<(), PathError> {
    let invalid = component.chars().find(|&c| match os {
        OperatingSystem::Windows => c < ' ' || matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'),
        _ => c == '\0',
    });
    match invalid {
        Some(character) => Err(PathError::InvalidCharacter {
            path: input.into(),
            character,
        }),
        None => Ok(()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    /// `C:foo`: relative to the current directory of a drive.
    DriveRelative(String),
    /// `\foo`: relative to the root of the current drive.
    NoDrive(String),
    InvalidUnc(String),
    /// `\\.\…` device paths and other verbatim forms.
    DevicePath(String),
    InvalidCharacter {
        path: String,
        character: char,
    },
    /// A relative path where no base directory is known.
    RelativeWithoutBase(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "empty path"),
            Self::DriveRelative(p) => write!(f, "drive-relative path '{p}'"),
            Self::NoDrive(p) => write!(f, "path '{p}' has no drive letter"),
            Self::InvalidUnc(p) => write!(f, "invalid UNC path '{p}'"),
            Self::DevicePath(p) => write!(f, "unsupported device path '{p}'"),
            Self::InvalidCharacter { path, character } => {
                write!(f, "invalid character {character:?} in path '{path}'")
            }
            Self::RelativeWithoutBase(p) => {
                write!(f, "relative path '{p}' without a base directory")
            }
        }
    }
}

impl std::error::Error for PathError {}

#[cfg(test)]
mod tests {
    use super::*;
    use OperatingSystem::{Linux, MacOs, Windows};

    fn p(os: OperatingSystem, input: &str) -> TargetPath {
        TargetPath::parse(os, input).unwrap()
    }

    fn display(os: OperatingSystem, input: &str) -> String {
        p(os, input).to_string()
    }

    #[test]
    fn normalises_unix_paths() {
        assert_eq!(display(Linux, "/home/alice/"), "/home/alice");
        assert_eq!(display(Linux, "/home//alice/./x/../y"), "/home/alice/y");
        assert_eq!(display(Linux, "/../.."), "/");
        assert_eq!(display(Linux, "/"), "/");
        assert_eq!(display(Linux, "a/../../b"), "../b");
        assert_eq!(display(Linux, "./"), ".");
    }

    #[test]
    fn unix_backslash_is_part_of_the_name() {
        let path = p(Linux, r"/srv/a\b");
        assert_eq!(path.components(), ["srv", r"a\b"]);
    }

    #[test]
    fn normalises_windows_drive_paths() {
        assert_eq!(display(Windows, r"c:\Users\Alice\"), r"C:\Users\Alice");
        assert_eq!(display(Windows, "C:/Users/alice/../bob"), r"C:\Users\bob");
        assert_eq!(display(Windows, r"C:\"), r"C:\");
        assert_eq!(display(Windows, r"C:\..\.."), r"C:\");
        assert!(p(Windows, r"D:\").is_drive_root());
    }

    #[test]
    fn parses_unc_paths() {
        let path = p(Windows, r"\\server\share\team\docs");
        assert_eq!(path.to_string(), r"\\server\share\team\docs");
        assert_eq!(path.components(), ["team", "docs"]);
        assert!(p(Windows, r"\\server\share").is_drive_root());
        assert!(p(Windows, "//server/share/x").is_absolute());
    }

    #[test]
    fn parses_verbatim_paths() {
        assert_eq!(display(Windows, r"\\?\C:\Temp"), r"C:\Temp");
        assert_eq!(display(Windows, r"\\?\UNC\srv\share\x"), r"\\srv\share\x");
    }

    #[test]
    fn rejects_invalid_windows_paths() {
        let err = |input| TargetPath::parse(Windows, input).unwrap_err();
        assert!(matches!(err(r"\\server"), PathError::InvalidUnc(_)));
        assert!(matches!(err(r"\\server\"), PathError::InvalidUnc(_)));
        assert!(matches!(err("C:foo"), PathError::DriveRelative(_)));
        assert!(matches!(err(r"\Windows"), PathError::NoDrive(_)));
        assert!(matches!(
            err(r"\\.\PhysicalDrive0"),
            PathError::DevicePath(_)
        ));
        assert!(matches!(
            err(r"C:\a|b"),
            PathError::InvalidCharacter { character: '|', .. }
        ));
        assert!(matches!(
            err(r"C:\a\b:c"),
            PathError::InvalidCharacter { .. }
        ));
        assert!(matches!(err(""), PathError::Empty));
    }

    #[test]
    fn rejects_nul_on_unix() {
        assert!(TargetPath::parse(Linux, "/a\0b").is_err());
    }

    #[test]
    fn keeps_unicode_and_spaces() {
        assert_eq!(
            display(MacOs, "/Users/jürgen/Mein Ordner/数据"),
            "/Users/jürgen/Mein Ordner/数据"
        );
        assert_eq!(
            display(Windows, r"C:\Users\Jürgen\Eigene Dateien"),
            r"C:\Users\Jürgen\Eigene Dateien"
        );
    }

    #[test]
    fn handles_very_long_paths() {
        let long = format!("/{}", vec!["segment"; 500].join("/"));
        let path = p(Linux, &long);
        assert_eq!(path.components().len(), 500);
        assert_eq!(path.to_string(), long);
    }

    #[test]
    fn joins_relative_and_absolute() {
        let home = p(Linux, "/home/alice");
        assert_eq!(
            home.join("projects/../x").unwrap().to_string(),
            "/home/alice/x"
        );
        assert_eq!(home.join("/etc").unwrap().to_string(), "/etc");
        let windows = p(Windows, r"C:\Users\alice");
        assert_eq!(
            windows.join("AppData/Roaming").unwrap().to_string(),
            r"C:\Users\alice\AppData\Roaming"
        );
        assert_eq!(windows.join(r"D:\data").unwrap().to_string(), r"D:\data");
    }

    #[test]
    fn letter_case_depends_on_the_operating_system() {
        assert!(p(Windows, r"C:\Users\Alice\Docs").starts_with(&p(Windows, r"c:\users\alice")));
        assert!(p(MacOs, "/Users/Alice/x").starts_with(&p(MacOs, "/users/alice")));
        assert!(!p(Linux, "/home/Alice/x").starts_with(&p(Linux, "/home/alice")));
        assert!(p(Windows, r"\\SRV\Share\x").starts_with(&p(Windows, r"\\srv\share")));
        assert_eq!(
            p(Windows, r"C:\A").comparison_key(),
            p(Windows, r"c:\a").comparison_key()
        );
    }

    #[test]
    fn starts_with_compares_whole_components() {
        assert!(!p(Linux, "/home/alice2").starts_with(&p(Linux, "/home/alice")));
        assert!(p(Linux, "/home/alice").starts_with(&p(Linux, "/")));
        assert!(!p(Windows, r"D:\x").starts_with(&p(Windows, r"C:\")));
        assert!(!p(Linux, "home").starts_with(&p(Linux, "/home")));
    }

    #[test]
    fn parent_stops_at_the_root() {
        assert_eq!(p(Linux, "/a/b").parent().unwrap().to_string(), "/a");
        assert!(p(Linux, "/").parent().is_none());
        assert!(p(Windows, r"C:\").parent().is_none());
    }
}
