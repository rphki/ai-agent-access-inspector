use std::path::PathBuf;
use std::process::{Command, Output};

use serde_json::Value;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ai-agent-access-inspector"))
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

#[test]
fn version_flag_prints_name_and_version() {
    let output = cli().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        stdout(&output).trim(),
        format!("ai-agent-access-inspector {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_flag_prints_usage() {
    let output = cli().arg("--help").output().unwrap();
    assert!(output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Usage:"));
    assert!(text.contains("scan"));
}

#[test]
fn unknown_argument_fails() {
    let output = cli().arg("--bogus").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("--bogus"));
}

#[test]
fn list_scanners_includes_demo() {
    let output = cli().arg("list-scanners").output().unwrap();
    assert!(output.status.success());
    assert!(stdout(&output).contains("demo"));
}

#[test]
fn demo_scan_prints_json_report() {
    let home = PathBuf::from(if cfg!(windows) {
        r"C:\Users\alice"
    } else {
        "/home/alice"
    });
    let output = cli()
        .args(["scan", "--agent", "demo", "--format", "json", "--home"])
        .arg(&home)
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));

    let report: Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(report["formatVersion"], "0.1");
    assert_eq!(report["tool"]["version"], env!("CARGO_PKG_VERSION"));

    let result = &report["results"][0];
    assert_eq!(result["scanner"]["id"], "demo");
    assert_eq!(result["detection"]["status"], "configured");
    let findings = result["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 4);
    assert_eq!(
        findings[0]["target"]["basePath"],
        home.join("projects").to_string_lossy().as_ref()
    );
}

#[test]
fn unknown_agent_fails_with_hint() {
    let output = cli().args(["scan", "--agent", "nope"]).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("unknown scanner 'nope'"));
}

#[test]
fn missing_fixture_fails() {
    let output = cli()
        .args(["scan", "--agent", "demo", "--fixture", "does/not/exist"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("invalid fixture"));
}
