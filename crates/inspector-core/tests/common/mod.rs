//! Shared helpers for the fixture tests of all scanners.

use std::path::{Path, PathBuf};

use inspector_core::context::ScanContext;
use inspector_core::model::ScanResult;
use inspector_core::scanner::{find_scanner, run_scan};

/// `fixtures/<name>` in the repository.
pub fn fixtures_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

pub fn scan(fixture: &Path, scanner: &str) -> ScanResult {
    let context = ScanContext::from_fixture(fixture).unwrap();
    let scanner = find_scanner(scanner).unwrap();
    run_scan(&context, &[scanner]).results.remove(0)
}

/// Fixture directories (those with a `fixture.json`), sorted.
pub fn fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("fixture.json").is_file())
        .collect();
    fixtures.sort();
    fixtures
}

/// Compares every fixture's scan with its `expected.json`, or rewrites the
/// expectations with `UPDATE_EXPECT=1`.
pub fn check_expectations(dir: &Path, scanner: &str, test: &str) {
    let update = std::env::var_os("UPDATE_EXPECT").is_some();
    let mut failures = Vec::new();
    for fixture in fixtures(dir) {
        let actual = serde_json::to_string_pretty(&scan(&fixture, scanner)).unwrap() + "\n";
        let expected_path = fixture.join("expected.json");
        if update {
            std::fs::write(&expected_path, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&expected_path).unwrap_or_default();
        if expected.replace("\r\n", "\n") != actual {
            failures.push(fixture.file_name().unwrap().to_string_lossy().into_owned());
        }
    }
    assert!(
        failures.is_empty(),
        "scan results differ from expected.json for: {failures:?}\n\
         Run `UPDATE_EXPECT=1 cargo test -p inspector-core --test {test}` \
         and review the diff if the change is intended."
    );
}

pub fn check_reproducible(dir: &Path, scanner: &str) {
    for fixture in fixtures(dir) {
        assert_eq!(
            scan(&fixture, scanner),
            scan(&fixture, scanner),
            "{}",
            fixture.display()
        );
    }
}

/// No invented secret appears in the scan output. The message names the
/// fixture only, never the value.
pub fn check_no_secrets(dir: &Path, scanner: &str, secrets: &[&str]) {
    for fixture in fixtures(dir) {
        let output = serde_json::to_string(&scan(&fixture, scanner)).unwrap();
        for secret in secrets {
            assert!(
                !output.contains(secret),
                "potential secret leaked in {}",
                fixture.display()
            );
        }
    }
}

/// Acceptance check of phase 6: no export of any fixture, in any format and
/// with any path privacy, contains a word that looks like a secret. With
/// `home` the home directory is gone, with `anonymized` the user name too.
pub fn check_exports(dir: &Path, scanner: &str) {
    use inspector_core::export::anonymize::{PathPrivacy, anonymize};
    use inspector_core::export::markdown::{self, MarkdownOptions};
    use inspector_core::redact::{REDACTED, looks_like_secret};

    for fixture in fixtures(dir) {
        let context = ScanContext::from_fixture(&fixture).unwrap();
        let report = run_scan(&context, &[find_scanner(scanner).unwrap()]);
        let home = context.home_dir.to_string_lossy().into_owned();
        let user = home.rsplit(['/', '\\']).next().unwrap().to_lowercase();
        let name = fixture.file_name().unwrap().to_string_lossy().into_owned();

        for privacy in [
            PathPrivacy::Full,
            PathPrivacy::Home,
            PathPrivacy::Anonymized,
        ] {
            let exported = anonymize(&report, &home, privacy);
            let markdown_options = MarkdownOptions {
                verbose: true,
                translations: None,
            };
            for output in [
                inspector_core::export::json(&exported),
                markdown::render(&exported, markdown_options),
            ] {
                let secrets: Vec<&str> = output
                    // Words as `redact_text` sees them; paths stay one word.
                    .split(|c: char| c.is_whitespace() || "\"'`(),;|<>[]{}".contains(c))
                    .filter(|word| *word != REDACTED && looks_like_secret(word))
                    .collect();
                assert!(
                    secrets.is_empty(),
                    "{name} {privacy:?}: found potential secret(s)"
                );
                let unescaped = output.replace("\\\\", "\\");
                if privacy != PathPrivacy::Full {
                    // As a path of its own; `~/.claude/home/alice` is a
                    // directory below the home, not the home.
                    let left = unescaped.match_indices(&home).any(|(i, _)| {
                        unescaped[..i]
                            .chars()
                            .next_back()
                            .is_none_or(|c| !c.is_alphanumeric() && !"._-/\\".contains(c))
                    });
                    assert!(!left, "{name} {privacy:?}: home left");
                }
                if privacy == PathPrivacy::Anonymized {
                    assert!(
                        !unescaped.to_lowercase().contains(&user),
                        "{name} {privacy:?}: user name left"
                    );
                }
            }
        }
    }
}
