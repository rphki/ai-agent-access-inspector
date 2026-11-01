//! Redaction of values that look like secrets.
//!
//! Applied to everything a scanner outputs that may carry credentials, such
//! as command line arguments of MCP servers.

pub const REDACTED: &str = "<redacted>";

/// Flags whose value is a secret (`--token abc` or `--token=abc`).
const SECRET_FLAGS: &[&str] = &[
    "--access-token",
    "--api-key",
    "--apikey",
    "--auth",
    "--auth-token",
    "--client-secret",
    "--key",
    "--password",
    "--secret",
    "--token",
];

/// Prefixes of well-known token formats.
const SECRET_PREFIXES: &[&str] = &[
    "AIza",
    "AKIA",
    "ghp_",
    "gho_",
    "ghs_",
    "ghu_",
    "github_pat_",
    "glpat-",
    "sk-",
    "xoxb-",
    "xoxp-",
];

/// URL query parameters that carry secrets.
const SECRET_PARAMETERS: &[&str] = &["token=", "key=", "password=", "secret=", "auth="];

/// Replaces secret-looking arguments with [`REDACTED`].
pub fn redact_args(args: &[String]) -> Vec<String> {
    let mut redacted = Vec::with_capacity(args.len());
    let mut next_is_secret = false;
    for arg in args {
        if next_is_secret {
            redacted.push(REDACTED.to_string());
            next_is_secret = false;
            continue;
        }
        let lower = arg.to_ascii_lowercase();
        if SECRET_FLAGS.contains(&lower.as_str()) {
            redacted.push(arg.clone());
            next_is_secret = true;
        } else if let Some((flag, _)) = arg
            .split_once('=')
            .filter(|(flag, _)| SECRET_FLAGS.contains(&flag.to_ascii_lowercase().as_str()))
        {
            redacted.push(format!("{flag}={REDACTED}"));
        } else if let Some((name, _)) = arg.split_once('=').filter(|(name, _)| is_secret_name(name))
        {
            redacted.push(format!("{name}={REDACTED}"));
        } else if looks_like_secret(arg) {
            redacted.push(REDACTED.to_string());
        } else {
            redacted.push(arg.clone());
        }
    }
    redacted
}

/// `API_KEY`, `GITHUB_TOKEN`, `DB_PASSWORD` and similar variable names.
fn is_secret_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    !name.is_empty()
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && [
            "TOKEN",
            "KEY",
            "SECRET",
            "PASSWORD",
            "PASSWD",
            "AUTH",
            "CREDENTIAL",
        ]
        .iter()
        .any(|word| upper.contains(word))
}

/// Heuristic: known token prefixes, secret URL parameters, or a long
/// random-looking string without path separators.
pub fn looks_like_secret(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    if SECRET_PREFIXES
        .iter()
        .any(|prefix| value.starts_with(prefix))
    {
        return true;
    }
    if value.contains("://") && SECRET_PARAMETERS.iter().any(|p| lower.contains(p)) {
        return true;
    }
    value.len() >= 32
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '='))
        && value.chars().any(|c| c.is_ascii_digit())
        && value.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn redacts_values_of_secret_flags() {
        assert_eq!(
            redact_args(&args(&[
                "--token",
                "abc",
                "--API-KEY=xyz",
                "--port",
                "8080"
            ])),
            args(&[
                "--token",
                REDACTED,
                "--API-KEY=<redacted>",
                "--port",
                "8080"
            ])
        );
    }

    #[test]
    fn redacts_token_like_values() {
        assert_eq!(
            redact_args(&args(&[
                "ghp_abcdefghijklmnopqrstuvwxyz0123456789",
                "sk-proj-abc",
                "a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8",
                "https://api.example.com/?token=abc",
            ])),
            args(&[REDACTED, REDACTED, REDACTED, REDACTED])
        );
    }

    #[test]
    fn redacts_secret_assignments() {
        assert_eq!(
            redact_args(&args(&["env", "GITHUB_TOKEN=abc", "PORT=8080", "server"])),
            args(&["env", "GITHUB_TOKEN=<redacted>", "PORT=8080", "server"])
        );
    }

    #[test]
    fn keeps_paths_packages_and_short_values() {
        let kept = args(&[
            "-y",
            "@modelcontextprotocol/server-filesystem",
            "/Users/alice/projects/a-very-long-directory-name-2026",
            r"C:\Users\alice\Documents",
            "server.py",
            "https://example.com/mcp",
        ]);
        assert_eq!(redact_args(&kept), kept);
    }
}
