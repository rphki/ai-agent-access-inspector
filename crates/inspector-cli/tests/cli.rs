use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_ai-agent-access-inspector"))
}

#[test]
fn version_flag_prints_name_and_version() {
    let output = cli().arg("--version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        stdout.trim(),
        format!("ai-agent-access-inspector {}", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn help_flag_prints_usage() {
    let output = cli().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout).unwrap().contains("Usage:"));
}

#[test]
fn unknown_argument_fails() {
    let output = cli().arg("--bogus").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--bogus")
    );
}
