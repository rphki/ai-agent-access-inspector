//! Checking paths on disk. Only metadata is read, never file contents.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::model::{Existence, PathKind};

/// State of a host path on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostState {
    pub existence: Existence,
    pub kind: PathKind,
    /// Set if the path itself is a symlink: the fully resolved target, or
    /// for dangling links and loops the link's immediate target.
    pub symlink_target: Option<PathBuf>,
}

/// Inspects `path` without following anything but the path's own symlink.
///
/// Missing paths, permission errors and symlink loops are reported in the
/// result; this function never fails.
pub fn inspect(path: &Path) -> HostState {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) => return unavailable(&error.kind()),
    };
    if !metadata.file_type().is_symlink() {
        return HostState {
            existence: Existence::Exists,
            kind: kind_of(&metadata),
            symlink_target: None,
        };
    }

    match fs::canonicalize(path) {
        Ok(target) => HostState {
            existence: Existence::Exists,
            kind: fs::metadata(&target)
                .map(|m| kind_of(&m))
                .unwrap_or(PathKind::Unknown),
            symlink_target: Some(target),
        },
        Err(error) => {
            // Dangling link or loop: report where the link points.
            let target = fs::read_link(path).ok().map(|link| match path.parent() {
                Some(parent) if link.is_relative() => parent.join(link),
                _ => link,
            });
            let existence = match error.kind() {
                ErrorKind::NotFound => Existence::Missing,
                _ => Existence::Unknown,
            };
            HostState {
                existence,
                kind: PathKind::Unknown,
                symlink_target: target,
            }
        }
    }
}

fn unavailable(kind: &ErrorKind) -> HostState {
    HostState {
        existence: match kind {
            ErrorKind::NotFound => Existence::Missing,
            _ => Existence::Unknown,
        },
        kind: PathKind::Unknown,
        symlink_target: None,
    }
}

fn kind_of(metadata: &fs::Metadata) -> PathKind {
    if metadata.is_dir() {
        PathKind::Directory
    } else if metadata.is_file() {
        PathKind::File
    } else {
        PathKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TempDir;

    #[test]
    fn reports_files_directories_and_missing_paths() {
        let dir = TempDir::new("fs-kinds");
        std::fs::write(dir.path().join("file.txt"), "x").unwrap();

        let directory = inspect(dir.path());
        assert_eq!(directory.existence, Existence::Exists);
        assert_eq!(directory.kind, PathKind::Directory);
        assert_eq!(directory.symlink_target, None);

        assert_eq!(inspect(&dir.path().join("file.txt")).kind, PathKind::File);

        let missing = inspect(&dir.path().join("nope"));
        assert_eq!(missing.existence, Existence::Missing);
        assert_eq!(missing.kind, PathKind::Unknown);
    }

    #[test]
    fn handles_paths_with_spaces_and_unicode() {
        let dir = TempDir::new("fs-unicode");
        let path = dir.path().join("Mein Ordner – Jürgen 数据");
        std::fs::create_dir(&path).unwrap();
        assert_eq!(inspect(&path).kind, PathKind::Directory);
    }

    #[cfg(unix)]
    #[test]
    fn resolves_symlinks() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new("fs-symlink");
        let target = dir.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let link = dir.path().join("link");
        symlink(&target, &link).unwrap();

        let state = inspect(&link);
        assert_eq!(state.existence, Existence::Exists);
        assert_eq!(state.kind, PathKind::Directory);
        assert_eq!(
            state.symlink_target,
            Some(std::fs::canonicalize(&target).unwrap())
        );
    }

    #[cfg(unix)]
    #[test]
    fn reports_dangling_symlinks_and_loops() {
        use std::os::unix::fs::symlink;

        let dir = TempDir::new("fs-dangling");
        let dangling = dir.path().join("dangling");
        symlink("gone", &dangling).unwrap();
        let state = inspect(&dangling);
        assert_eq!(state.existence, Existence::Missing);
        assert_eq!(state.symlink_target, Some(dir.path().join("gone")));

        let a = dir.path().join("a");
        let b = dir.path().join("b");
        symlink(&b, &a).unwrap();
        symlink(&a, &b).unwrap();
        let state = inspect(&a);
        assert_eq!(state.existence, Existence::Unknown);
        assert_eq!(state.symlink_target, Some(b));
    }
}
