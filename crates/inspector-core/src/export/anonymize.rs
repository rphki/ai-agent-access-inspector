//! Path privacy for exports: the home directory as `~` and anonymised user
//! names (roadmap, section 13).
//!
//! Works on every string of the serialised report, so paths inside message
//! texts and parameters are covered as well as path fields.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::ScanReport;
use crate::platform::OperatingSystem;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathPrivacy {
    /// Paths as they are.
    #[default]
    Full,
    /// The home directory is shown as `~`.
    Home,
    /// Like `Home`, and user names in user directories (`/Users/<name>`,
    /// `/home/<name>`, `C:\Users\<name>`) become `user`, `user2`, ….
    Anonymized,
}

/// Name used for the scanned user; other users get a number.
const OWN_USER: &str = "user";

/// Directories whose next component is a user name.
const USER_DIRECTORIES: &[&str] = &["/Users/", "/home/", "\\Users\\"];

/// The same directories in Claude Code's project directory names, where
/// separators become dashes: `~/.claude/projects/-home-alice-app/`.
const ENCODED_USER_DIRECTORIES: &[&str] = &["-Users-", "-home-"];

/// Returns a copy of the report with the requested path privacy.
///
/// Except for `Full`, finding ids are renumbered (`finding-1`, …), because
/// the regular ids are hashes over paths and could confirm a guessed path.
pub fn anonymize(report: &ScanReport, home: &str, privacy: PathPrivacy) -> ScanReport {
    if privacy == PathPrivacy::Full {
        return report.clone();
    }
    let mut report = report.clone();
    renumber_ids(&mut report);

    let windows = report.operating_system == OperatingSystem::Windows;
    let mut rewriter = Rewriter {
        windows,
        homes: home_variants(home, windows),
        own_user: last_component(home).map(str::to_string),
        users: BTreeMap::new(),
        anonymize_users: privacy == PathPrivacy::Anonymized,
    };
    let mut value = serde_json::to_value(&report).expect("a scan report always serialises");
    rewriter.visit(&mut value);
    serde_json::from_value(value).expect("rewriting strings keeps the report's shape")
}

fn renumber_ids(report: &mut ScanReport) {
    let mut ids = BTreeMap::new();
    let findings = report
        .results
        .iter_mut()
        .flat_map(|r| r.findings.iter_mut());
    for (index, finding) in findings.enumerate() {
        let new = format!("finding-{}", index + 1);
        ids.insert(std::mem::replace(&mut finding.id, new.clone()), new);
    }
    for finding in report
        .results
        .iter_mut()
        .flat_map(|r| r.findings.iter_mut())
    {
        for related in &mut finding.related {
            if let Some(new) = ids.get(&related.finding_id) {
                related.finding_id = new.clone();
            }
        }
    }
}

/// The home directory as it can appear in the report: on Windows with
/// either separator.
fn home_variants(home: &str, windows: bool) -> Vec<String> {
    let home = home.trim_end_matches(['/', '\\']);
    if home.is_empty() {
        return Vec::new();
    }
    let mut variants = vec![home.to_string()];
    if windows {
        let slashed = home.replace('\\', "/");
        if slashed != home {
            variants.push(slashed);
        }
    }
    variants
}

fn last_component(home: &str) -> Option<&str> {
    home.trim_end_matches(['/', '\\'])
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
}

/// Characters that can be part of a path component.
fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '$' | '@' | '+')
}

struct Rewriter {
    windows: bool,
    homes: Vec<String>,
    own_user: Option<String>,
    /// Other user names and their replacement, in order of appearance.
    users: BTreeMap<String, String>,
    anonymize_users: bool,
}

impl Rewriter {
    fn visit(&mut self, value: &mut Value) {
        match value {
            Value::String(text) => *text = self.rewrite(text),
            Value::Array(items) => items.iter_mut().for_each(|item| self.visit(item)),
            Value::Object(fields) => {
                for (name, field) in fields.iter_mut() {
                    match field {
                        Value::String(pointer) if name == "pointer" => {
                            *pointer = self.rewrite_pointer(pointer);
                        }
                        _ => self.visit(field),
                    }
                }
            }
            _ => {}
        }
    }

    fn rewrite(&mut self, text: &str) -> String {
        let mut text = text.to_string();
        for home in self.homes.clone() {
            text = self.replace_home(&text, &home);
        }
        if self.anonymize_users {
            for directory in USER_DIRECTORIES {
                text = self.replace_user_names(&text, directory);
            }
            for directory in ENCODED_USER_DIRECTORIES {
                text = self.replace_encoded_user_names(&text, directory);
            }
        }
        text
    }

    /// JSON pointers escape `/` in keys as `~1`, e.g. `/projects/~1home~1alice`;
    /// each key is rewritten on its own.
    fn rewrite_pointer(&mut self, pointer: &str) -> String {
        pointer
            .split('/')
            .map(|key| {
                let key = key.replace("~1", "/").replace("~0", "~");
                self.rewrite(&key).replace('~', "~0").replace('/', "~1")
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Positions of `needle` in `text`, ignoring ASCII case on Windows.
    fn find_all(&self, text: &str, needle: &str) -> Vec<usize> {
        let (haystack, needle) = if self.windows {
            (text.to_ascii_lowercase(), needle.to_ascii_lowercase())
        } else {
            (text.to_string(), needle.to_string())
        };
        haystack.match_indices(&needle).map(|(i, _)| i).collect()
    }

    /// Replaces the home directory with `~` where it is a whole path prefix:
    /// not preceded by a path character and followed by a separator or the
    /// end of the path (`/home/al` does not match `/home/alice`).
    fn replace_home(&self, text: &str, home: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut copied = 0;
        for start in self.find_all(text, home) {
            let end = start + home.len();
            if start < copied {
                continue;
            }
            let before = text[..start].chars().next_back();
            let after = &text[end..];
            let starts_path = before.is_none_or(|c| !is_name_char(c) && c != '/' && c != '\\');
            if starts_path && ends_component(after) {
                out.push_str(&text[copied..start]);
                out.push('~');
                copied = end;
            }
        }
        out.push_str(&text[copied..]);
        out
    }

    /// Replaces the component after a user directory (e.g. `/home/`).
    fn replace_user_names(&mut self, text: &str, directory: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut copied = 0;
        for start in self.find_all(text, directory) {
            let name_start = start + directory.len();
            if name_start < copied {
                continue;
            }
            let name_len: usize = text[name_start..]
                .chars()
                .take_while(|&c| is_name_char(c))
                .map(char::len_utf8)
                .sum();
            let name = &text[name_start..name_start + name_len];
            // `/home/x.` at the end of a sentence: the dot is punctuation.
            let name = name.trim_end_matches('.');
            if name.is_empty() || !ends_component(&text[name_start + name.len()..]) {
                continue;
            }
            let alias = self.alias(name);
            out.push_str(&text[copied..name_start]);
            out.push_str(&alias);
            copied = name_start + name.len();
        }
        out.push_str(&text[copied..]);
        out
    }

    /// Like `replace_user_names` for dash-encoded paths. A dash may belong to
    /// the name or be a separator, so the own user name is matched as a
    /// whole first; other names end at the next dash.
    fn replace_encoded_user_names(&mut self, text: &str, directory: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut copied = 0;
        for start in self.find_all(text, directory) {
            let name_start = start + directory.len();
            if name_start < copied {
                continue;
            }
            let rest = &text[name_start..];
            let own_len = self
                .own_user
                .as_deref()
                .filter(|own| {
                    let prefix = rest.get(..own.len());
                    let matches = if self.windows {
                        prefix.is_some_and(|p| p.eq_ignore_ascii_case(own))
                    } else {
                        prefix == Some(*own)
                    };
                    matches
                        && rest[own.len()..]
                            .chars()
                            .next()
                            .is_none_or(|c| !is_name_char(c) || c == '-')
                })
                .map(str::len);
            let name_len = own_len.unwrap_or_else(|| {
                rest.chars()
                    .take_while(|&c| is_name_char(c) && c != '-' && c != '.')
                    .map(char::len_utf8)
                    .sum()
            });
            if name_len == 0 {
                continue;
            }
            let alias = self.alias(&rest[..name_len]);
            out.push_str(&text[copied..name_start]);
            out.push_str(&alias);
            copied = name_start + name_len;
        }
        out.push_str(&text[copied..]);
        out
    }

    fn alias(&mut self, name: &str) -> String {
        let key = if self.windows {
            name.to_lowercase()
        } else {
            name.to_string()
        };
        let own = self.own_user.as_deref().is_some_and(|own| {
            if self.windows {
                own.to_lowercase() == key
            } else {
                own == key
            }
        });
        if own {
            return OWN_USER.to_string();
        }
        let next = self.users.len() + 2;
        self.users
            .entry(key)
            .or_insert_with(|| format!("{OWN_USER}{next}"))
            .clone()
    }
}

/// Whether a path component ends here: end of text, a separator, or a
/// character that cannot belong to a path component. A dot only ends it at
/// the end of a sentence.
fn ends_component(after: &str) -> bool {
    let mut chars = after.chars();
    match chars.next() {
        None => true,
        Some('.') => chars.next().is_none_or(char::is_whitespace),
        Some(c) => !is_name_char(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ScanResult, ToolInfo};

    fn report(os: OperatingSystem, fixture: &str) -> ScanReport {
        let path = format!(
            "{}/../../fixtures/claude-code/{fixture}/expected.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let result: ScanResult =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        ScanReport {
            format_version: "test".into(),
            tool: ToolInfo::current(),
            operating_system: os,
            results: vec![result],
        }
    }

    fn rewriter(os: OperatingSystem, home: &str, privacy: PathPrivacy) -> Rewriter {
        let windows = os == OperatingSystem::Windows;
        Rewriter {
            windows,
            homes: home_variants(home, windows),
            own_user: last_component(home).map(str::to_string),
            users: BTreeMap::new(),
            anonymize_users: privacy == PathPrivacy::Anonymized,
        }
    }

    #[test]
    fn full_keeps_everything() {
        let original = report(OperatingSystem::Linux, "linux-basic");
        assert_eq!(
            anonymize(&original, "/home/alice", PathPrivacy::Full),
            original
        );
    }

    #[test]
    fn home_becomes_tilde_on_component_boundaries() {
        let mut r = rewriter(OperatingSystem::MacOs, "/Users/alice", PathPrivacy::Home);
        assert_eq!(r.rewrite("/Users/alice"), "~");
        assert_eq!(r.rewrite("/Users/alice/app/**"), "~/app/**");
        assert_eq!(r.rewrite("Grants /Users/alice."), "Grants ~.");
        assert_eq!(r.rewrite("Read(//Users/alice/x)"), "Read(//Users/alice/x)");
        assert_eq!(r.rewrite("/Users/alicebob/x"), "/Users/alicebob/x");
        assert_eq!(r.rewrite("/Users/alice.bak"), "/Users/alice.bak");
        assert_eq!(r.rewrite("/Users/al"), "/Users/al");
        assert_eq!(r.rewrite("/Users/bob"), "/Users/bob");
    }

    #[test]
    fn windows_homes_with_both_separators_and_any_case() {
        let mut r = rewriter(
            OperatingSystem::Windows,
            r"C:\Users\Alice",
            PathPrivacy::Home,
        );
        assert_eq!(r.rewrite(r"C:\Users\Alice\notes"), r"~\notes");
        assert_eq!(r.rewrite(r"c:\users\alice\notes"), r"~\notes");
        assert_eq!(r.rewrite("C:/Users/Alice/notes"), "~/notes");
    }

    #[test]
    fn anonymized_replaces_user_names_in_user_directories() {
        let mut r = rewriter(OperatingSystem::Linux, "/home/dev", PathPrivacy::Anonymized);
        assert_eq!(r.rewrite("/home/dev/app"), "~/app");
        assert_eq!(r.rewrite("/dev/null"), "/dev/null");
        assert_eq!(r.rewrite("/srv/dev/home"), "/srv/dev/home");
        assert_eq!(
            r.rewrite("/home/bob/x and /home/carol"),
            "/home/user2/x and /home/user3"
        );
        assert_eq!(r.rewrite("/home/bob"), "/home/user2");
        // The own name in a user directory that is not the home, e.g. a mount.
        assert_eq!(r.rewrite("/mnt/home/dev/x"), "/mnt/home/user/x");
    }

    #[test]
    fn anonymized_windows_and_mac_user_directories() {
        let mut r = rewriter(
            OperatingSystem::Windows,
            r"C:\Users\alice",
            PathPrivacy::Anonymized,
        );
        assert_eq!(r.rewrite(r"D:\Users\Bob\x"), r"D:\Users\user2\x");
        assert_eq!(
            r.rewrite(r"\\server\share\Users\bob"),
            r"\\server\share\Users\user2"
        );
        let mut r = rewriter(
            OperatingSystem::MacOs,
            "/Users/alice",
            PathPrivacy::Anonymized,
        );
        assert_eq!(
            r.rewrite("/Volumes/x/Users/alice/y"),
            "/Volumes/x/Users/user/y"
        );
    }

    #[test]
    fn anonymized_covers_encoded_project_directories() {
        let mut r = rewriter(
            OperatingSystem::Linux,
            "/home/al-ice",
            PathPrivacy::Anonymized,
        );
        assert_eq!(
            r.rewrite("~/.claude/projects/-home-al-ice-work-app/x.jsonl"),
            "~/.claude/projects/-home-user-work-app/x.jsonl"
        );
        assert_eq!(r.rewrite("-home-bob-app"), "-home-user2-app");
        assert_eq!(r.rewrite("my-home-made-tool"), "my-home-user3-tool");
    }

    #[test]
    fn json_pointers_are_rewritten_per_key() {
        let mut r = rewriter(
            OperatingSystem::Linux,
            "/home/alice",
            PathPrivacy::Anonymized,
        );
        assert_eq!(
            r.rewrite_pointer("/projects/~1home~1alice~1app/allowedTools/0"),
            "/projects/~0~1app/allowedTools/0"
        );
        assert_eq!(
            r.rewrite_pointer("/permissions/allow/3"),
            "/permissions/allow/3"
        );
    }

    #[test]
    fn ids_are_renumbered_consistently() {
        let original = report(OperatingSystem::Linux, "linux-basic");
        let exported = anonymize(&original, "/home/alice", PathPrivacy::Home);
        let findings = &exported.results[0].findings;
        assert_eq!(findings[0].id, "finding-1");
        let ids: Vec<&str> = findings.iter().map(|f| f.id.as_str()).collect();
        for finding in findings {
            for related in &finding.related {
                assert!(
                    ids.contains(&related.finding_id.as_str()),
                    "{}",
                    related.finding_id
                );
            }
        }
        assert!(findings.iter().any(|f| !f.related.is_empty()));
    }

    #[test]
    fn no_home_or_user_name_survives_in_any_fixture() {
        for (fixture, os, home, user) in [
            (
                "linux-basic",
                OperatingSystem::Linux,
                "/home/alice",
                "alice",
            ),
            (
                "macos-managed",
                OperatingSystem::MacOs,
                "/Users/alice",
                "alice",
            ),
            (
                "windows-basic",
                OperatingSystem::Windows,
                r"C:\Users\alice",
                "alice",
            ),
            ("secrets", OperatingSystem::Linux, "/home/carol", "carol"),
        ] {
            let original = report(os, fixture);
            let home_only =
                serde_json::to_string(&anonymize(&original, home, PathPrivacy::Home)).unwrap();
            assert!(
                !home_only.contains(&home.replace('\\', "\\\\")),
                "{fixture}: home left"
            );
            let anonymous =
                serde_json::to_string(&anonymize(&original, home, PathPrivacy::Anonymized))
                    .unwrap();
            assert!(
                !anonymous.to_lowercase().contains(user),
                "{fixture}: user name left"
            );
        }
    }
}
