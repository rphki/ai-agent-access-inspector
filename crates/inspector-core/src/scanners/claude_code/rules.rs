//! Claude Code permission rules: `Tool` or `Tool(specifier)`.
//!
//! Path rules for `Read` and `Edit` use gitignore patterns with four
//! anchors (see `docs/claude-code-scanner.md`, section 6):
//! `//path` absolute, `~/path` home, `/path` relative to the settings
//! source, `path` or `./path` relative to the working directory.

use crate::analysis::path::{PathError, TargetPath};
use crate::analysis::pattern::split_pattern;
use crate::analysis::resolve::resolve_path;
use crate::model::{AccessKind, PathAnchor, PatternKind};
use crate::platform::OperatingSystem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rule<'a> {
    /// `Read(…)` or `Edit(…)`.
    Path {
        tool: PathTool,
        spec: &'a str,
    },
    /// A tool without specifier, e.g. `Bash` or `Bash(*)`.
    WholeTool(&'a str),
    /// A path rule for a tool whose path rules never match.
    Ineffective {
        tool: &'a str,
    },
    /// Any other valid rule, e.g. `Bash(npm run *)` or `WebFetch(…)`.
    Other,
    Unparsable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathTool {
    Read,
    Edit,
}

impl PathTool {
    pub fn access(self) -> AccessKind {
        match self {
            Self::Read => AccessKind::Read,
            // Edit rules cover all file-writing tools.
            Self::Edit => AccessKind::Write,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Edit => "edit",
        }
    }
}

pub fn parse_rule(rule: &str) -> Rule<'_> {
    let rule = rule.trim();
    let (tool, spec) = match rule.find('(') {
        Some(open) => {
            let Some(spec) = rule[open + 1..].strip_suffix(')') else {
                return Rule::Unparsable;
            };
            (&rule[..open], Some(spec))
        }
        None => (rule, None),
    };
    let valid_name = !tool.is_empty()
        && tool
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '*'));
    if !valid_name {
        return Rule::Unparsable;
    }
    match (tool, spec) {
        (_, None) | ("Bash", Some("*")) => Rule::WholeTool(tool),
        (_, Some("")) => Rule::Unparsable,
        ("Read", Some(spec)) => Rule::Path {
            tool: PathTool::Read,
            spec,
        },
        ("Edit", Some(spec)) => Rule::Path {
            tool: PathTool::Edit,
            spec,
        },
        ("Write" | "NotebookEdit" | "Glob" | "MultiEdit", Some(_)) => Rule::Ineffective { tool },
        _ => Rule::Other,
    }
}

/// A path pattern resolved against the scanned system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPattern {
    pub negated: bool,
    pub kind: PatternKind,
    pub anchor: PathAnchor,
    /// `None` if the pattern is relative to a working directory that is not
    /// known at this level (user or managed settings).
    pub base: Option<TargetPath>,
    /// `/Users/…` and similar: a single slash that was probably meant as an
    /// absolute path.
    pub likely_misanchored: bool,
}

/// Where relative parts of a rule are anchored.
pub struct Anchors<'a> {
    pub home: &'a TargetPath,
    /// Base for `/path`: the project for project settings, `~/.claude` for
    /// user settings.
    pub settings: Option<&'a TargetPath>,
    /// Base for `path` and `./path`: the project, if known.
    pub working_directory: Option<&'a TargetPath>,
}

/// Top-level directories that suggest a single-slash pattern was meant to be
/// absolute.
const ROOT_DIRECTORIES: &[&str] = &[
    "Applications",
    "Library",
    "System",
    "Users",
    "Volumes",
    "etc",
    "home",
    "mnt",
    "opt",
    "private",
    "root",
    "srv",
    "tmp",
    "usr",
    "var",
];

pub fn resolve_rule_path(spec: &str, anchors: &Anchors) -> Result<ResolvedPattern, PathError> {
    let os = anchors.home.os();
    let parts = split_pattern(spec);
    let pattern = spec.strip_prefix('!').unwrap_or(spec);
    let base = parts.base.as_str();

    let (anchor, resolved, likely_misanchored) = if pattern.starts_with("//") {
        let absolute = base.strip_prefix('/').unwrap_or(base);
        (
            PathAnchor::FilesystemRoot,
            Some(absolute_posix(os, absolute)?),
            false,
        )
    } else if pattern == "~" || pattern.starts_with("~/") {
        (
            PathAnchor::Home,
            Some(resolve_path(base, anchors.home, None)?),
            false,
        )
    } else if pattern.starts_with('/') {
        let relative = base.trim_start_matches('/');
        let first = relative.split('/').next().unwrap_or_default();
        let misanchored = ROOT_DIRECTORIES.contains(&first)
            || (os == OperatingSystem::Windows
                && first.len() == 1
                && first.chars().all(|c| c.is_ascii_alphabetic()));
        let resolved = match anchors.settings {
            Some(dir) => Some(join_relative(dir, relative)?),
            None => None,
        };
        let anchor = match anchors.settings {
            Some(dir) => PathAnchor::Directory {
                path: dir.to_path_buf(),
            },
            None => PathAnchor::WorkingDirectory,
        };
        (anchor, resolved, misanchored)
    } else {
        let relative = base.strip_prefix("./").unwrap_or(base);
        let resolved = match anchors.working_directory {
            Some(dir) => Some(join_relative(dir, relative)?),
            None => None,
        };
        (PathAnchor::WorkingDirectory, resolved, false)
    };

    Ok(ResolvedPattern {
        negated: parts.negated,
        kind: parts.kind,
        anchor,
        base: resolved,
        likely_misanchored,
    })
}

fn join_relative(dir: &TargetPath, relative: &str) -> Result<TargetPath, PathError> {
    if relative.is_empty() || relative == "." {
        Ok(dir.clone())
    } else {
        dir.join(relative)
    }
}

/// Converts the POSIX form Claude Code uses for absolute paths (`/Users/x`,
/// on Windows `/c/Users/x`) into a target path.
pub fn absolute_posix(os: OperatingSystem, path: &str) -> Result<TargetPath, PathError> {
    let path = path.trim_start_matches('/');
    match os {
        OperatingSystem::Windows => {
            let mut parts = path.splitn(2, '/');
            let first = parts.next().unwrap_or_default();
            let rest = parts.next().unwrap_or_default();
            if first.len() == 1 && first.chars().all(|c| c.is_ascii_alphabetic()) {
                TargetPath::parse(os, &format!("{first}:\\{rest}"))
            } else {
                TargetPath::parse(os, &format!("\\\\{path}"))
            }
        }
        _ => TargetPath::parse(os, &format!("/{path}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use OperatingSystem::{Linux, Windows};

    #[test]
    fn parses_rule_kinds() {
        assert_eq!(
            parse_rule("Read(~/x/**)"),
            Rule::Path {
                tool: PathTool::Read,
                spec: "~/x/**"
            }
        );
        assert_eq!(
            parse_rule(" Edit(/src/**) "),
            Rule::Path {
                tool: PathTool::Edit,
                spec: "/src/**"
            }
        );
        assert_eq!(parse_rule("Bash"), Rule::WholeTool("Bash"));
        assert_eq!(parse_rule("Bash(*)"), Rule::WholeTool("Bash"));
        assert_eq!(parse_rule("Bash(npm run *)"), Rule::Other);
        assert_eq!(parse_rule("WebFetch(domain:example.com)"), Rule::Other);
        assert_eq!(
            parse_rule("mcp__github__*"),
            Rule::WholeTool("mcp__github__*")
        );
        assert_eq!(parse_rule("Write(/x)"), Rule::Ineffective { tool: "Write" });
        assert_eq!(parse_rule("Read(~/x"), Rule::Unparsable);
        assert_eq!(parse_rule("Read()"), Rule::Unparsable);
        assert_eq!(parse_rule("no spaces allowed"), Rule::Unparsable);
    }

    struct Setup {
        home: TargetPath,
        claude_dir: TargetPath,
        project: TargetPath,
    }

    fn setup(os: OperatingSystem) -> Setup {
        let (home, project) = match os {
            Windows => (r"C:\Users\alice", r"C:\Users\alice\app"),
            _ => ("/home/alice", "/home/alice/app"),
        };
        let home = TargetPath::parse(os, home).unwrap();
        Setup {
            claude_dir: home.join(".claude").unwrap(),
            project: TargetPath::parse(os, project).unwrap(),
            home,
        }
    }

    fn resolve(setup: &Setup, spec: &str, project_level: bool) -> ResolvedPattern {
        let anchors = Anchors {
            home: &setup.home,
            settings: Some(if project_level {
                &setup.project
            } else {
                &setup.claude_dir
            }),
            working_directory: project_level.then_some(&setup.project),
        };
        resolve_rule_path(spec, &anchors).unwrap()
    }

    fn base(pattern: &ResolvedPattern) -> Option<String> {
        pattern.base.as_ref().map(ToString::to_string)
    }

    #[test]
    fn resolves_the_four_anchors() {
        let s = setup(Linux);
        let absolute = resolve(&s, "//srv/data/**", false);
        assert_eq!(base(&absolute).as_deref(), Some("/srv/data"));
        assert_eq!(absolute.anchor, PathAnchor::FilesystemRoot);
        assert_eq!(absolute.kind, PatternKind::Directory);

        assert_eq!(
            base(&resolve(&s, "~/docs/*.pdf", false)).as_deref(),
            Some("/home/alice/docs")
        );
        assert_eq!(
            base(&resolve(&s, "/secrets/**", false)).as_deref(),
            Some("/home/alice/.claude/secrets")
        );
        assert_eq!(
            base(&resolve(&s, "/src/**", true)).as_deref(),
            Some("/home/alice/app/src")
        );
        assert_eq!(
            base(&resolve(&s, "./.env", true)).as_deref(),
            Some("/home/alice/app/.env")
        );
        assert_eq!(
            base(&resolve(&s, "*.env", true)).as_deref(),
            Some("/home/alice/app")
        );
        assert_eq!(base(&resolve(&s, "//**", false)).as_deref(), Some("/"));
    }

    #[test]
    fn relative_patterns_in_user_settings_have_no_base() {
        let s = setup(Linux);
        let pattern = resolve(&s, "secrets/**", false);
        assert_eq!(pattern.base, None);
        assert_eq!(pattern.anchor, PathAnchor::WorkingDirectory);
    }

    #[test]
    fn detects_likely_misanchored_patterns() {
        let s = setup(Linux);
        assert!(resolve(&s, "/Users/alice/x", false).likely_misanchored);
        assert!(resolve(&s, "/home/alice/x/**", false).likely_misanchored);
        assert!(!resolve(&s, "/src/**", true).likely_misanchored);
        assert!(!resolve(&s, "//Users/alice/x", false).likely_misanchored);
    }

    #[test]
    fn reports_negation() {
        let s = setup(Linux);
        let pattern = resolve(&s, "!sample.env", true);
        assert!(pattern.negated);
        assert_eq!(
            base(&pattern).as_deref(),
            Some("/home/alice/app/sample.env")
        );
    }

    #[test]
    fn converts_posix_forms_on_windows() {
        let s = setup(Windows);
        assert_eq!(
            base(&resolve(&s, "//c/Users/alice/**/.env", false)).as_deref(),
            Some(r"C:\Users\alice")
        );
        assert_eq!(base(&resolve(&s, "//d/**", false)).as_deref(), Some(r"D:\"));
        assert_eq!(
            base(&resolve(&s, "~/Documents/**", false)).as_deref(),
            Some(r"C:\Users\alice\Documents")
        );
        assert_eq!(
            base(&resolve(&s, "/src/**", true)).as_deref(),
            Some(r"C:\Users\alice\app\src")
        );
        assert!(resolve(&s, "/c/Users/x", false).likely_misanchored);
        assert_eq!(
            absolute_posix(Windows, "//server/share/x")
                .unwrap()
                .to_string(),
            r"\\server\share\x"
        );
    }
}
