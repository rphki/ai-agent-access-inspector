//! Runs the ChatGPT/Codex scanner on every fixture in `fixtures/codex/`
//! and compares the result with the fixture's `expected.json`.
//!
//! After an intended change, regenerate the expectations with
//! `UPDATE_EXPECT=1 cargo test -p inspector-core --test codex_fixtures`
//! and review the diff.

mod common;

const SCANNER: &str = "codex";

fn dir() -> std::path::PathBuf {
    common::fixtures_dir("codex")
}

#[test]
fn fixtures_match_expectations() {
    common::check_expectations(&dir(), SCANNER, "codex_fixtures");
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
        // All invented secrets of the fixtures start with `fixture-`.
        &["fixture-"],
    );
}

/// Acceptance check of phase 6: no export of any fixture, in any format and
/// with any path privacy, contains a word that looks like a secret.
#[test]
fn exports_contain_no_secrets_home_or_user_names() {
    common::check_exports(&dir(), SCANNER);
}
