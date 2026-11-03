//! Runs the Claude Code scanner on every fixture in `fixtures/claude-code/`
//! and compares the result with the fixture's `expected.json`.
//!
//! After an intended change, regenerate the expectations with
//! `UPDATE_EXPECT=1 cargo test -p inspector-core --test claude_code_fixtures`
//! and review the diff.

mod common;

const SCANNER: &str = "claude-code";

fn dir() -> std::path::PathBuf {
    common::fixtures_dir("claude-code")
}

#[test]
fn fixtures_match_expectations() {
    common::check_expectations(&dir(), SCANNER, "claude_code_fixtures");
}

#[test]
fn scans_are_reproducible() {
    common::check_reproducible(&dir(), SCANNER);
}

#[test]
fn no_secret_from_any_fixture_appears_in_the_output() {
    common::check_no_secrets(
        &dir(),
        SCANNER,
        &[
            "never-shown",
            "should-never-be-read",
            "never kept",
            "never read",
            "abc123",
            "ghp_",
            // All invented secrets of the fixture `secrets`.
            "fixture-",
        ],
    );
}

/// Acceptance check of phase 6: no export of any fixture, in any format and
/// with any path privacy, contains a word that looks like a secret.
#[test]
fn exports_contain_no_secrets_home_or_user_names() {
    common::check_exports(&dir(), SCANNER);
}
