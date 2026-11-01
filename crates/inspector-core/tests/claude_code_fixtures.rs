//! Runs the Claude Code scanner on every fixture in `fixtures/claude-code/`
//! and compares the result with the fixture's `expected.json`.
//!
//! After an intended change, regenerate the expectations with
//! `UPDATE_EXPECT=1 cargo test -p inspector-core --test claude_code_fixtures`
//! and review the diff.

use std::path::{Path, PathBuf};

use inspector_core::context::ScanContext;
use inspector_core::model::ScanResult;
use inspector_core::scanner::{find_scanner, run_scan};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/claude-code")
}

fn scan(fixture: &Path) -> ScanResult {
    let context = ScanContext::from_fixture(fixture).unwrap();
    let scanner = find_scanner("claude-code").unwrap();
    run_scan(&context, &[scanner]).results.remove(0)
}

fn fixtures() -> Vec<PathBuf> {
    let mut fixtures: Vec<PathBuf> = std::fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("fixture.json").is_file())
        .collect();
    fixtures.sort();
    fixtures
}

#[test]
fn fixtures_match_expectations() {
    let update = std::env::var_os("UPDATE_EXPECT").is_some();
    let mut failures = Vec::new();
    for fixture in fixtures() {
        let actual = serde_json::to_string_pretty(&scan(&fixture)).unwrap() + "\n";
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
         Run `UPDATE_EXPECT=1 cargo test -p inspector-core --test claude_code_fixtures` \
         and review the diff if the change is intended."
    );
}

#[test]
fn scans_are_reproducible() {
    for fixture in fixtures() {
        assert_eq!(scan(&fixture), scan(&fixture), "{}", fixture.display());
    }
}

#[test]
fn no_secret_from_any_fixture_appears_in_the_output() {
    for fixture in fixtures() {
        let output = serde_json::to_string(&scan(&fixture)).unwrap();
        for secret in [
            "never-shown",
            "should-never-be-read",
            "never kept",
            "never read",
            "abc123",
            "ghp_",
            // All invented secrets of the fixture `secrets`.
            "fixture-",
        ] {
            assert!(
                !output.contains(secret),
                "{secret} leaked in {}",
                fixture.display()
            );
        }
    }
}

/// Acceptance check of phase 6: no export of any fixture, in any format and
/// with any path privacy, contains a word that looks like a secret. With
/// `home` the home directory is gone, with `anonymized` the user name too.
#[test]
fn exports_contain_no_secrets_home_or_user_names() {
    use inspector_core::export::anonymize::{PathPrivacy, anonymize};
    use inspector_core::export::markdown::{self, MarkdownOptions};
    use inspector_core::redact::{REDACTED, looks_like_secret};

    for fixture in fixtures() {
        let context = ScanContext::from_fixture(&fixture).unwrap();
        let report = run_scan(&context, &[find_scanner("claude-code").unwrap()]);
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
                assert!(secrets.is_empty(), "{name} {privacy:?}: {secrets:?}");
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
