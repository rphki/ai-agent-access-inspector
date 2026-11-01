//! Turning configured path values into absolute target paths.

use crate::context::EnvSnapshot;

use super::path::{PathError, TargetPath};

/// Result of expanding environment variables in a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expansion {
    pub value: String,
    /// Variables that were neither set nor had a default; they stay literal.
    pub unresolved: Vec<String>,
}

/// Expands `${VAR}` and `${VAR:-default}`.
///
/// A variable set to an empty string counts as set and does not fall back
/// to the default. Unset variables without a default stay literal and are
/// reported in [`Expansion::unresolved`]. Other forms (`$VAR`, `%VAR%`) are
/// left untouched.
pub fn expand_env(input: &str, env: &EnvSnapshot) -> Expansion {
    let mut value = String::with_capacity(input.len());
    let mut unresolved = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find("${") {
        value.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            value.push_str(&rest[start..]);
            rest = "";
            break;
        };
        let inner = &after[..end];
        let (name, default) = match inner.split_once(":-") {
            Some((name, default)) => (name, Some(default)),
            None => (inner, None),
        };
        if is_variable_name(name) {
            match (env.get(name), default) {
                (Some(set), _) => value.push_str(set),
                (None, Some(default)) => value.push_str(default),
                (None, None) => {
                    value.push_str(&rest[start..start + 3 + end]);
                    unresolved.push(name.to_string());
                }
            }
        } else {
            value.push_str(&rest[start..start + 3 + end]);
        }
        rest = &after[end + 1..];
    }
    value.push_str(rest);
    Expansion { value, unresolved }
}

fn is_variable_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Resolves a configured path to an absolute target path.
///
/// `~` and `~/…` (on Windows also `~\…`) are resolved against `home`,
/// relative paths against `base`. Environment variables must be expanded
/// beforehand with [`expand_env`].
pub fn resolve_path(
    input: &str,
    home: &TargetPath,
    base: Option<&TargetPath>,
) -> Result<TargetPath, PathError> {
    if input == "~" {
        return Ok(home.clone());
    }
    let windows = home.os() == crate::platform::OperatingSystem::Windows;
    if let Some(rest) = input
        .strip_prefix("~/")
        .or_else(|| input.strip_prefix("~\\").filter(|_| windows))
    {
        return home.join(rest);
    }
    let path = TargetPath::parse(home.os(), input)?;
    if path.is_absolute() {
        return Ok(path);
    }
    match base {
        Some(base) => base.join(input),
        None => Err(PathError::RelativeWithoutBase(input.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::OperatingSystem::{Linux, Windows};

    fn env(pairs: &[(&str, &str)]) -> EnvSnapshot {
        let mut env = EnvSnapshot::default();
        for (name, value) in pairs {
            env.set(*name, *value);
        }
        env
    }

    #[test]
    fn expands_set_variables_and_defaults() {
        let env = env(&[("DATA", "/srv/data"), ("EMPTY", "")]);
        assert_eq!(expand_env("${DATA}/x", &env).value, "/srv/data/x");
        assert_eq!(expand_env("${MISSING:-/tmp}/x", &env).value, "/tmp/x");
        assert_eq!(expand_env("a${EMPTY:-fallback}b", &env).value, "ab");
        assert_eq!(
            expand_env("${DATA}:${DATA}", &env).value,
            "/srv/data:/srv/data"
        );
    }

    #[test]
    fn keeps_unset_variables_literal() {
        let expansion = expand_env("${NOPE}/x", &EnvSnapshot::default());
        assert_eq!(expansion.value, "${NOPE}/x");
        assert_eq!(expansion.unresolved, ["NOPE"]);
    }

    #[test]
    fn ignores_other_variable_forms_and_broken_syntax() {
        let env = env(&[("HOME", "/home/alice")]);
        assert_eq!(expand_env("$HOME/%HOME%", &env).value, "$HOME/%HOME%");
        assert_eq!(expand_env("${HOME", &env).value, "${HOME");
        assert_eq!(expand_env("${1BAD}", &env).value, "${1BAD}");
        assert!(expand_env("${1BAD}", &env).unresolved.is_empty());
    }

    #[test]
    fn resolves_home_relative_and_absolute_paths() {
        let home = TargetPath::parse(Linux, "/home/alice").unwrap();
        let project = TargetPath::parse(Linux, "/home/alice/app").unwrap();
        let resolve = |input| {
            resolve_path(input, &home, Some(&project))
                .unwrap()
                .to_string()
        };
        assert_eq!(resolve("~"), "/home/alice");
        assert_eq!(resolve("~/docs/../x"), "/home/alice/x");
        assert_eq!(resolve("/etc"), "/etc");
        assert_eq!(resolve("src"), "/home/alice/app/src");
        assert_eq!(resolve("../other"), "/home/alice/other");
        assert_eq!(resolve("~alice"), "/home/alice/app/~alice");
    }

    #[test]
    fn resolves_windows_home_with_both_separators() {
        let home = TargetPath::parse(Windows, r"C:\Users\alice").unwrap();
        let resolve = |input| resolve_path(input, &home, None).unwrap().to_string();
        assert_eq!(resolve("~/Documents"), r"C:\Users\alice\Documents");
        assert_eq!(resolve(r"~\Documents"), r"C:\Users\alice\Documents");
    }

    #[test]
    fn relative_path_without_base_is_an_error() {
        let home = TargetPath::parse(Linux, "/home/alice").unwrap();
        assert_eq!(
            resolve_path("src", &home, None),
            Err(PathError::RelativeWithoutBase("src".into()))
        );
    }
}
