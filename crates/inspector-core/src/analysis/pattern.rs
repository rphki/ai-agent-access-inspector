//! Splitting gitignore-style path patterns into a base path and the rest.

use crate::model::PatternKind;

/// A pattern split at its first component that contains a glob character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternParts {
    /// The pattern started with `!` (a gitignore negation).
    pub negated: bool,
    /// Everything before the first glob component, without a trailing `/`.
    /// Empty if the pattern starts with a glob.
    pub base: String,
    pub kind: PatternKind,
}

const GLOB_CHARACTERS: &[char] = &['*', '?', '[', '{'];

/// Splits `pattern` (components separated by `/`).
///
/// - no glob character: `ExactPath` (or `Directory` with a trailing `/`)
/// - only a trailing `/**`: `Directory`, everything below the base
/// - anything else: `Glob`
pub fn split_pattern(pattern: &str) -> PatternParts {
    let (negated, pattern) = match pattern.strip_prefix('!') {
        Some(rest) => (true, rest),
        None => (false, pattern),
    };
    let components: Vec<&str> = pattern.split('/').collect();
    let first_glob = components.iter().position(|c| c.contains(GLOB_CHARACTERS));

    let Some(index) = first_glob else {
        let kind = if pattern.len() > 1 && pattern.ends_with('/') {
            PatternKind::Directory
        } else {
            PatternKind::ExactPath
        };
        return PatternParts {
            negated,
            base: trim_trailing_separator(pattern).into(),
            kind,
        };
    };

    let base = components[..index].join("/");
    let base = if base.is_empty() && pattern.starts_with('/') {
        "/".to_string()
    } else {
        trim_trailing_separator(&base).to_string()
    };
    let kind = if index == components.len() - 1 && components[index] == "**" {
        PatternKind::Directory
    } else {
        PatternKind::Glob
    };
    PatternParts {
        negated,
        base,
        kind,
    }
}

fn trim_trailing_separator(path: &str) -> &str {
    if path.len() > 1 {
        path.strip_suffix('/').unwrap_or(path)
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(pattern: &str) -> (bool, String, PatternKind) {
        let parts = split_pattern(pattern);
        (parts.negated, parts.base, parts.kind)
    }

    #[test]
    fn exact_paths_and_directories() {
        assert_eq!(
            split("~/.zshrc"),
            (false, "~/.zshrc".into(), PatternKind::ExactPath)
        );
        assert_eq!(
            split("secrets/"),
            (false, "secrets".into(), PatternKind::Directory)
        );
        assert_eq!(split("/"), (false, "/".into(), PatternKind::ExactPath));
    }

    #[test]
    fn trailing_double_star_is_a_directory() {
        assert_eq!(
            split("~/projects/**"),
            (false, "~/projects".into(), PatternKind::Directory)
        );
        assert_eq!(split("//**"), (false, "/".into(), PatternKind::Directory));
        assert_eq!(split("**"), (false, String::new(), PatternKind::Directory));
    }

    #[test]
    fn other_globs_keep_the_prefix_as_base() {
        assert_eq!(
            split("~/projects/**/*.md"),
            (false, "~/projects".into(), PatternKind::Glob)
        );
        assert_eq!(
            split("//c/**/.env"),
            (false, "//c".into(), PatternKind::Glob)
        );
        assert_eq!(split("*.env"), (false, String::new(), PatternKind::Glob));
        assert_eq!(
            split("src/[ab]/x"),
            (false, "src".into(), PatternKind::Glob)
        );
        assert_eq!(
            split("/data/file?.txt"),
            (false, "/data".into(), PatternKind::Glob)
        );
    }

    #[test]
    fn negation_is_reported() {
        assert_eq!(
            split("!sample.env"),
            (true, "sample.env".into(), PatternKind::ExactPath)
        );
    }
}
