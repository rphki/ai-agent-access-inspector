//! Discovering the projects Claude Code knows about.
//!
//! Sources: the current directory of the scan, the `projects` map of
//! `.claude.json` and the session logs below `~/.claude/projects/`. The last
//! two are heuristic (see the specification, sections 3.3 and 3.5).

use std::collections::HashMap;
use std::time::SystemTime;

use serde::Deserialize;
use serde_json::Value;

use super::{ClaudeJson, Locations, Scan, pointer_key};
use crate::analysis::path::TargetPath;
use crate::context::ScanContext;
use crate::model::{
    AccessKind, Confidence, Diagnostic, DiagnosticCode, DiagnosticLevel, Finding, FindingScope,
    FindingStatus, FindingTarget, Message, PatternKind, SourceDocumentation,
};
use crate::read;

/// How many session directories are examined at most.
const MAX_SESSION_DIRS: usize = 1_000;

/// Maximum depth when looking for the Git repository root.
const MAX_GIT_DEPTH: usize = 64;

pub(crate) struct Project {
    pub path: TargetPath,
    /// The project's trust dialog has been accepted.
    pub trusted: bool,
    /// The project's key and entry in `.claude.json`, if any.
    pub key: Option<String>,
    pub entry: Option<Value>,
}

/// Fields read from a session log line; everything else is discarded.
#[derive(Deserialize)]
pub(crate) struct SessionLine {
    pub cwd: Option<String>,
    pub version: Option<String>,
}

pub(crate) struct Session {
    pub log: TargetPath,
    pub cwd: String,
    pub version: Option<String>,
    modified: Option<SystemTime>,
}

pub(crate) fn discover(scan: &mut Scan, claude_json: Option<&ClaudeJson>) -> Vec<Project> {
    let os = scan.os();
    let mut projects: Vec<Project> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut add = |projects: &mut Vec<Project>, path: TargetPath| -> usize {
        *index.entry(path.comparison_key()).or_insert_with(|| {
            projects.push(Project {
                path,
                trusted: false,
                key: None,
                entry: None,
            });
            projects.len() - 1
        })
    };

    if let Some(current) = scan
        .context
        .current_dir
        .as_ref()
        .and_then(|dir| TargetPath::parse(os, &dir.to_string_lossy()).ok())
        .filter(TargetPath::is_absolute)
    {
        add(&mut projects, current);
    }

    if let Some(claude_json) = claude_json {
        for (key, entry) in &claude_json.projects {
            let Some(path) = TargetPath::parse(os, key)
                .ok()
                .filter(TargetPath::is_absolute)
            else {
                continue;
            };
            let i = add(&mut projects, path.clone());
            projects[i].trusted = entry.get("hasTrustDialogAccepted") == Some(&Value::Bool(true));
            projects[i].key = Some(key.clone());
            projects[i].entry = Some(entry.clone());
            if entry
                .get("allowedTools")
                .and_then(Value::as_array)
                .is_some_and(|tools| !tools.is_empty())
            {
                scan.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Info,
                        DiagnosticCode::IgnoredSetting,
                        Message::new("diagnostic.legacyAllowedTools").arg("project", &path),
                    )
                    .file(claude_json.path.to_path_buf())
                    .pointer(format!("/projects/{}/allowedTools", pointer_key(key))),
                );
            }
            let source = scan.source(
                "claude-json",
                SourceDocumentation::Heuristic,
                &claude_json.path,
                &format!("/projects/{}", pointer_key(key)),
            );
            scan.finding(known_project(scan, &path, source.raw_value(key)));
        }
    }

    for session in sessions(scan.context, &scan.locations) {
        let Some(path) = TargetPath::parse(os, &session.cwd)
            .ok()
            .filter(TargetPath::is_absolute)
        else {
            continue;
        };
        add(&mut projects, path.clone());
        let source = scan.source(
            "session",
            SourceDocumentation::Heuristic,
            &session.log,
            "/cwd",
        );
        scan.finding(known_project(scan, &path, source.raw_value(&session.cwd)));
    }
    projects
}

fn known_project(scan: &Scan, path: &TargetPath, source: crate::model::FindingSource) -> Finding {
    Finding::builder(
        &scan.meta,
        FindingScope::Project,
        FindingTarget::path(path.to_string(), PatternKind::Directory, path.to_path_buf()),
        source,
    )
    .access(AccessKind::ReadWrite)
    .status(FindingStatus::Observed)
    .confidence(Confidence::Medium)
    .explanation(Message::new("finding.knownProject"))
    .build()
}

/// The most recent session log of every session directory.
pub(crate) fn sessions(context: &ScanContext, locations: &Locations) -> Vec<Session> {
    let dir = locations.sessions_dir();
    let mut sessions = Vec::new();
    for name in read::list_dir(context, &dir.to_string())
        .into_iter()
        .take(MAX_SESSION_DIRS)
    {
        let Ok(project_dir) = dir.join(&name) else {
            continue;
        };
        if !read::is_dir(context, &project_dir.to_string()) {
            continue;
        }
        let newest = read::list_dir(context, &project_dir.to_string())
            .into_iter()
            .filter(|file| file.ends_with(".jsonl"))
            .filter_map(|file| project_dir.join(&file).ok())
            .map(|log| (read::modified(context, &log.to_string()), log))
            .max_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| a.1.to_string().cmp(&b.1.to_string()))
            });
        let Some((modified, log)) = newest else {
            continue;
        };
        let line: Option<SessionLine> =
            read::first_json_line(context, &log.to_string(), |line: &SessionLine| {
                line.cwd.is_some()
            });
        if let Some(SessionLine {
            cwd: Some(cwd),
            version,
        }) = line
        {
            sessions.push(Session {
                log,
                cwd,
                version,
                modified,
            });
        }
    }
    sessions
}

/// The most recently written session, used for the Claude Code version.
pub(crate) fn latest_session(context: &ScanContext, locations: &Locations) -> Option<Session> {
    sessions(context, locations)
        .into_iter()
        .max_by(|a, b| a.modified.cmp(&b.modified))
}

/// The root of the Git repository containing `path`, if any.
pub(crate) fn git_root(context: &ScanContext, path: &TargetPath) -> Option<TargetPath> {
    let mut current = Some(path.clone());
    for _ in 0..MAX_GIT_DEPTH {
        let dir = current?;
        if read::exists(context, &dir.join(".git").ok()?.to_string()) {
            return Some(dir);
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::context::EnvSnapshot;
    use crate::platform::OperatingSystem;
    use crate::test_support::TempDir;

    #[test]
    fn finds_the_git_root_above_a_project() {
        let dir = TempDir::new("git-root");
        let context = ScanContext {
            home_dir: PathBuf::from("/home/alice"),
            current_dir: None,
            operating_system: OperatingSystem::Linux,
            env: EnvSnapshot::default(),
            system_root: Some(dir.path().join("system")),
        };
        std::fs::create_dir_all(context.host_path("/home/alice/repo/.git")).unwrap();
        std::fs::create_dir_all(context.host_path("/home/alice/repo/packages/app")).unwrap();
        let project =
            TargetPath::parse(OperatingSystem::Linux, "/home/alice/repo/packages/app").unwrap();
        assert_eq!(
            git_root(&context, &project)
                .map(|p| p.to_string())
                .as_deref(),
            Some("/home/alice/repo")
        );
        let outside = TargetPath::parse(OperatingSystem::Linux, "/srv/other").unwrap();
        assert_eq!(git_root(&context, &outside), None);
    }
}
