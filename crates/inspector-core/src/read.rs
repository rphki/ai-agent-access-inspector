//! Reading files of the scanned system safely.
//!
//! Configuration files are untrusted input. Every read goes through this
//! module, which only opens regular files (no devices, pipes or sockets),
//! enforces a size limit and turns every problem into a [`Diagnostic`].

use std::fs::{self, File};
use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::context::ScanContext;
use crate::model::{Diagnostic, DiagnosticCode, DiagnosticLevel, Message};

/// Largest configuration file that is read. `~/.claude.json` can grow to a
/// few megabytes; anything far beyond that is not a configuration file.
pub const MAX_FILE_SIZE: u64 = 16 * 1024 * 1024;

/// How much of a session log is read at most to find one field.
pub const MAX_LOG_PREFIX: u64 = 4 * 1024 * 1024;

/// Upper bound for entries taken from one directory listing.
pub const MAX_DIR_ENTRIES: usize = 10_000;

#[derive(Debug)]
pub enum JsonFile {
    Missing,
    Parsed(Value),
    Failed(Diagnostic),
}

/// Reads and parses a JSON file given as target path.
pub fn read_json(context: &ScanContext, target: &str) -> JsonFile {
    match read_text(context, target) {
        Ok(None) => JsonFile::Missing,
        Err(diagnostic) => JsonFile::Failed(*diagnostic),
        Ok(Some(text)) => match serde_json::from_str(&text) {
            Ok(value) => JsonFile::Parsed(value),
            Err(error) => JsonFile::Failed(
                Diagnostic::new(
                    DiagnosticLevel::Error,
                    DiagnosticCode::InvalidJson,
                    Message::new("diagnostic.invalidJson").arg("error", error),
                )
                .file(target),
            ),
        },
    }
}

/// Reads a text file. `Ok(None)` if it does not exist.
pub fn read_text(context: &ScanContext, target: &str) -> Result<Option<String>, Box<Diagnostic>> {
    let host = context.host_path(target);
    let metadata = match fs::metadata(&host) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Box::new(unreadable(target, &error))),
    };
    if !metadata.is_file() {
        return Err(Box::new(
            Diagnostic::new(
                DiagnosticLevel::Warning,
                DiagnosticCode::UnreadableFile,
                Message::new("diagnostic.notARegularFile"),
            )
            .file(target),
        ));
    }
    if metadata.len() > MAX_FILE_SIZE {
        return Err(Box::new(too_large(target)));
    }
    let mut text = String::new();
    File::open(&host)
        .and_then(|file| file.take(MAX_FILE_SIZE + 1).read_to_string(&mut text))
        .map_err(|error| Box::new(unreadable(target, &error)))?;
    // The file may have grown between the size check and the read.
    if text.len() as u64 > MAX_FILE_SIZE {
        return Err(Box::new(too_large(target)));
    }
    Ok(Some(text))
}

/// Returns the first line of a JSON-lines file that parses as `T` and
/// satisfies `accept`. Reads at most [`MAX_LOG_PREFIX`] bytes; lines that do
/// not parse are skipped. Parsed lines are dropped right away.
pub fn first_json_line<T: DeserializeOwned>(
    context: &ScanContext,
    target: &str,
    accept: impl Fn(&T) -> bool,
) -> Option<T> {
    let host = context.host_path(target);
    if !fs::metadata(&host).ok()?.is_file() {
        return None;
    }
    let mut reader = BufReader::new(File::open(&host).ok()?.take(MAX_LOG_PREFIX));
    let mut line = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line).ok()? == 0 {
            return None;
        }
        if let Ok(value) = serde_json::from_slice::<T>(&line) {
            if accept(&value) {
                return Some(value);
            }
        }
    }
}

pub fn exists(context: &ScanContext, target: &str) -> bool {
    fs::symlink_metadata(context.host_path(target)).is_ok()
}

pub fn is_dir(context: &ScanContext, target: &str) -> bool {
    fs::metadata(context.host_path(target)).is_ok_and(|m| m.is_dir())
}

/// Names of the entries of a directory, sorted. Empty if it cannot be read.
pub fn list_dir(context: &ScanContext, target: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(context.host_path(target)) else {
        return Vec::new();
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .take(MAX_DIR_ENTRIES)
        .collect();
    names.sort();
    names
}

/// Modification time of a file, used to pick the most recent session log.
pub fn modified(context: &ScanContext, target: &str) -> Option<std::time::SystemTime> {
    fs::metadata(context.host_path(target))
        .ok()?
        .modified()
        .ok()
}

fn unreadable(target: &str, error: &std::io::Error) -> Diagnostic {
    Diagnostic::new(
        DiagnosticLevel::Error,
        DiagnosticCode::UnreadableFile,
        Message::new("diagnostic.unreadableFile").arg("error", error),
    )
    .file(PathBuf::from(target))
}

fn too_large(target: &str) -> Diagnostic {
    Diagnostic::new(
        DiagnosticLevel::Error,
        DiagnosticCode::FileTooLarge,
        Message::new("diagnostic.fileTooLarge")
            .arg("limit", format!("{} MiB", MAX_FILE_SIZE >> 20)),
    )
    .file(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::EnvSnapshot;
    use crate::platform::OperatingSystem;
    use crate::test_support::TempDir;

    fn system_context() -> ScanContext {
        ScanContext {
            home_dir: PathBuf::from("/home/alice"),
            current_dir: None,
            operating_system: OperatingSystem::current(),
            env: EnvSnapshot::default(),
            system_root: None,
        }
    }

    fn path(dir: &TempDir, name: &str) -> String {
        dir.path().join(name).to_string_lossy().into_owned()
    }

    #[test]
    fn parses_valid_json_and_reports_missing_files() {
        let dir = TempDir::new("read-json");
        std::fs::write(dir.path().join("ok.json"), r#"{"a": 1}"#).unwrap();
        let context = system_context();
        assert!(
            matches!(read_json(&context, &path(&dir, "ok.json")), JsonFile::Parsed(v) if v["a"] == 1)
        );
        assert!(matches!(
            read_json(&context, &path(&dir, "nope.json")),
            JsonFile::Missing
        ));
    }

    #[test]
    fn invalid_and_truncated_json_become_diagnostics() {
        let dir = TempDir::new("read-invalid");
        std::fs::write(
            dir.path().join("bad.json"),
            r#"{"permissions": {"allow": ["Read(~/x)""#,
        )
        .unwrap();
        let JsonFile::Failed(diagnostic) = read_json(&system_context(), &path(&dir, "bad.json"))
        else {
            panic!("expected a diagnostic");
        };
        assert_eq!(diagnostic.code, DiagnosticCode::InvalidJson);
        assert!(
            diagnostic.message.text.contains("EOF"),
            "{}",
            diagnostic.message.text
        );
    }

    #[test]
    fn directories_are_not_read() {
        let dir = TempDir::new("read-dir");
        let JsonFile::Failed(diagnostic) =
            read_json(&system_context(), &dir.path().to_string_lossy())
        else {
            panic!("expected a diagnostic");
        };
        assert_eq!(diagnostic.message.key, "diagnostic.notARegularFile");
    }

    #[cfg(unix)]
    #[test]
    fn device_files_are_not_read() {
        let JsonFile::Failed(diagnostic) = read_json(&system_context(), "/dev/zero") else {
            panic!("expected a diagnostic");
        };
        assert_eq!(diagnostic.message.key, "diagnostic.notARegularFile");
    }

    #[test]
    fn oversized_files_are_rejected() {
        let dir = TempDir::new("read-large");
        let file = std::fs::File::create(dir.path().join("big.json")).unwrap();
        file.set_len(MAX_FILE_SIZE + 1).unwrap();
        let JsonFile::Failed(diagnostic) = read_json(&system_context(), &path(&dir, "big.json"))
        else {
            panic!("expected a diagnostic");
        };
        assert_eq!(diagnostic.code, DiagnosticCode::FileTooLarge);
    }

    #[test]
    fn finds_the_first_matching_json_line() {
        #[derive(serde::Deserialize)]
        struct Line {
            cwd: Option<String>,
        }
        let dir = TempDir::new("read-lines");
        std::fs::write(
            dir.path().join("log.jsonl"),
            "not json\n{\"type\":\"x\"}\n{\"cwd\":\"/srv/app\",\"secret\":\"s\"}\n{\"cwd\":\"/other\"}\n",
        )
        .unwrap();
        let line: Line =
            first_json_line(&system_context(), &path(&dir, "log.jsonl"), |l: &Line| {
                l.cwd.is_some()
            })
            .unwrap();
        assert_eq!(line.cwd.as_deref(), Some("/srv/app"));
    }

    #[test]
    fn lists_directories_sorted() {
        let dir = TempDir::new("read-list");
        for name in ["b.json", "a.json", ".hidden"] {
            std::fs::write(dir.path().join(name), "{}").unwrap();
        }
        assert_eq!(
            list_dir(&system_context(), &dir.path().to_string_lossy()),
            [".hidden", "a.json", "b.json"]
        );
    }
}
