//! Redaction of values that look like secrets.
//!
//! Applied to everything a scanner outputs that may carry credentials:
//! command line arguments of MCP servers by the scanner, and all free text of
//! a scan result (rules, values, message parameters) by [`redact_result`].

use crate::model::{FindingSource, Message, ScanResult};

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

/// HTTP headers whose value is a secret (`Authorization: Bearer abc`).
const SECRET_HEADERS: &[&str] = &[
    "authorization:",
    "proxy-authorization:",
    "x-api-key:",
    "api-key:",
    "x-auth-token:",
];

/// Authentication schemes; the token after them is a secret.
const AUTH_SCHEMES: &[&str] = &["bearer", "basic", "token"];

/// Replaces secret-looking arguments with [`REDACTED`]. An argument that
/// contains several words, e.g. a header, is redacted with [`redact_text`].
pub fn redact_args(args: &[String]) -> Vec<String> {
    let mut redacted = Vec::with_capacity(args.len());
    let mut next_is_secret = false;
    for arg in args {
        let lower = arg.to_ascii_lowercase();
        if next_is_secret {
            // `Authorization: Bearer abc`: keep the scheme, redact the token.
            if AUTH_SCHEMES.contains(&lower.as_str()) {
                redacted.push(arg.clone());
            } else {
                redacted.push(REDACTED.to_string());
                next_is_secret = false;
            }
            continue;
        }
        if arg.chars().any(is_separator) {
            redacted.push(redact_text(arg));
        } else if SECRET_FLAGS.contains(&lower.as_str())
            || SECRET_HEADERS.contains(&lower.as_str())
            || lower == "bearer"
        {
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

/// Characters that separate words in free text. They are kept as they are.
fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '(' | ')' | ',' | ';')
}

/// Redacts secret-looking words in free text, e.g. a permission rule
/// `Bash(curl -H "Authorization: Bearer abc" …)`. Separators are kept, so
/// text without secrets is returned unchanged.
pub fn redact_text(text: &str) -> String {
    // Alternating runs of words and separators.
    let mut pieces: Vec<(bool, String)> = Vec::new();
    for c in text.chars() {
        let word = !is_separator(c);
        match pieces.last_mut() {
            Some((is_word, piece)) if *is_word == word => piece.push(c),
            _ => pieces.push((word, c.to_string())),
        }
    }
    let words: Vec<String> = pieces
        .iter()
        .filter(|(is_word, _)| *is_word)
        .map(|(_, piece)| piece.clone())
        .collect();
    let mut redacted = redact_args(&words).into_iter();
    pieces
        .into_iter()
        .map(|(is_word, piece)| {
            if is_word {
                redacted.next().unwrap_or(piece)
            } else {
                piece
            }
        })
        .collect()
}

/// Redacts all free text of a scan result: rule patterns, values from
/// configuration files and message parameters. Paths and ids are kept.
pub fn redact_result(result: &mut ScanResult) {
    for finding in &mut result.findings {
        finding.target.raw = redact_text(&finding.target.raw);
        finding.target.display_path = redact_text(&finding.target.display_path);
        redact_source(&mut finding.source);
        finding
            .additional_sources
            .iter_mut()
            .for_each(redact_source);
        redact_message(&mut finding.explanation);
        for warning in &mut finding.warnings {
            redact_message(&mut warning.message);
        }
    }
    for diagnostic in &mut result.diagnostics {
        redact_message(&mut diagnostic.message);
    }
    for entry in &mut result.summary {
        redact_message(&mut entry.label);
        redact_message(&mut entry.value);
    }
}

fn redact_source(source: &mut FindingSource) {
    for value in [&mut source.raw_value, &mut source.excerpt]
        .into_iter()
        .flatten()
    {
        *value = redact_text(value);
    }
}

/// Redacts the parameters and renders the text again if one changed.
fn redact_message(message: &mut Message) {
    let redacted: Vec<(String, String)> = message
        .args
        .iter()
        .map(|(name, value)| (name.clone(), redact_text(value)))
        .collect();
    if redacted
        .iter()
        .all(|(name, value)| &message.args[name] == value)
    {
        return;
    }
    let mut rendered = Message::new(&message.key);
    for (name, value) in redacted {
        rendered = rendered.arg(&name, value);
    }
    *message = rendered;
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
    fn redacts_headers_and_bearer_tokens() {
        assert_eq!(
            redact_args(&args(&[
                "--header",
                "Authorization: Bearer abc123",
                "-H",
                "X-Api-Key: xyz"
            ])),
            args(&[
                "--header",
                "Authorization: Bearer <redacted>",
                "-H",
                "X-Api-Key: <redacted>"
            ])
        );
    }

    #[test]
    fn redacts_secrets_in_free_text() {
        assert_eq!(
            redact_text(r#"Bash(curl -H "Authorization: Bearer abc123" https://api.example.com)"#),
            r#"Bash(curl -H "Authorization: Bearer <redacted>" https://api.example.com)"#
        );
        assert_eq!(
            redact_text("Bash(GITHUB_TOKEN=ghp_x gh pr list --token=abc)"),
            "Bash(GITHUB_TOKEN=<redacted> gh pr list --token=<redacted>)"
        );
        assert_eq!(
            redact_text("Bash(export KEY sk-proj-1)"),
            "Bash(export KEY <redacted>)"
        );
    }

    #[test]
    fn keeps_text_without_secrets_unchanged() {
        for text in [
            "Read(//Users/alice/My Documents/**)",
            "Bash(npm run test:*)",
            "WebFetch(domain:example.com)",
            "Grants the whole drive D:\\.",
            "",
        ] {
            assert_eq!(redact_text(text), text);
        }
        // Applying it twice changes nothing.
        let once = redact_text("Bash(curl --token abc)");
        assert_eq!(redact_text(&once), once);
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
