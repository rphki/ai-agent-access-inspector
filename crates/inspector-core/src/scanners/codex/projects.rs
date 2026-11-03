//! Working directories: trusted projects from `config.toml`, projects added
//! in the ChatGPT app and folders of chats without a project (app state).

use serde_json::Value;

use super::sandbox::{Mode, Policies};
use super::{Scan, View, pointer_key};
use crate::analysis::path::TargetPath;
use crate::model::{
    AccessKind, Confidence, Finding, FindingScope, FindingSource, FindingStatus, FindingTarget,
    Message, PatternKind, RuleEffect, SourceDocumentation,
};

/// Upper bound for paths taken from one key of the app state.
const MAX_APP_PATHS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    /// `[projects."<path>"]` in a configuration file.
    Config,
    /// `local-projects.<id>.rootPaths` in the app state.
    App,
    /// `thread-workspace-root-hints` in the app state.
    Observed,
}

#[derive(Debug, Clone)]
pub(crate) struct Project {
    pub path: TargetPath,
    /// `trust_level = "trusted"`; only then project configuration is loaded.
    pub trusted: bool,
    /// Has a `[projects."<path>"]` entry (trusted or not); without one Codex
    /// starts `read-only`.
    pub listed: bool,
    pub origin: Origin,
    pub source: FindingSource,
}

/// The permission mode selected in the app for the local machine.
#[derive(Debug, Clone)]
pub(crate) struct AppMode {
    pub value: String,
    pub source: FindingSource,
}

const APP_MODE_POINTER: &str = "/electron-persisted-atom-state/agent-mode-by-host-id/local";

/// Paths the app wrote for chats without a project.
#[derive(Debug, Clone)]
pub(crate) struct OutputDirectory {
    pub path: TargetPath,
    pub source: FindingSource,
}

pub(crate) fn discover(scan: &mut Scan, global: &View) -> (Vec<Project>, Option<AppMode>) {
    let mut projects: Vec<Project> = Vec::new();
    for layer in global.layers() {
        let Some(table) = layer.value.get("projects").and_then(Value::as_object) else {
            continue;
        };
        for (key, entry) in table {
            let pointer = format!("/projects/{}", pointer_key(key));
            let Some(path) = scan.target_path(key) else {
                continue;
            };
            let trusted = entry.get("trust_level").and_then(Value::as_str) == Some("trusted");
            let source = scan
                .source(
                    layer.level.config_kind(),
                    SourceDocumentation::Documented,
                    &layer.path,
                    &pointer,
                )
                .raw_value(key);
            add(&mut projects, path, trusted, Origin::Config, source);
        }
    }

    let state = scan.locations.app_state();
    let Some(value) = scan.read_json(&state) else {
        return (projects, None);
    };
    if let Some(local) = value.get("local-projects").and_then(Value::as_object) {
        for (id, project) in local.iter().take(MAX_APP_PATHS) {
            let Some(roots) = project.get("rootPaths").and_then(Value::as_array) else {
                continue;
            };
            for (i, root) in roots.iter().enumerate() {
                let Some(raw) = root.as_str() else { continue };
                let Some(path) = scan.target_path(raw) else {
                    continue;
                };
                let pointer = format!("/local-projects/{}/rootPaths/{i}", pointer_key(id));
                let source = scan
                    .source(
                        "app-state",
                        SourceDocumentation::Heuristic,
                        &state,
                        &pointer,
                    )
                    .raw_value(raw);
                add(&mut projects, path, false, Origin::App, source);
            }
        }
    }
    for (pointer, raw) in string_values(&value, "thread-workspace-root-hints") {
        let Some(path) = scan.target_path(&raw) else {
            continue;
        };
        let source = scan
            .source(
                "app-state",
                SourceDocumentation::Heuristic,
                &state,
                &pointer,
            )
            .raw_value(&raw);
        add(&mut projects, path, false, Origin::Observed, source);
    }
    let outputs: Vec<OutputDirectory> =
        string_values(&value, "thread-projectless-output-directories")
            .into_iter()
            .filter_map(|(pointer, raw)| {
                let path = scan.target_path(&raw)?;
                let source = scan
                    .source(
                        "app-state",
                        SourceDocumentation::Heuristic,
                        &state,
                        &pointer,
                    )
                    .raw_value(&raw);
                Some(OutputDirectory { path, source })
            })
            .collect();
    let app_mode = value
        .pointer(APP_MODE_POINTER)
        .and_then(Value::as_str)
        .map(|mode| AppMode {
            value: mode.to_string(),
            source: scan
                .source(
                    "app-state",
                    SourceDocumentation::Heuristic,
                    &state,
                    APP_MODE_POINTER,
                )
                .raw_value(mode),
        });
    // The rest of the app state is dropped here.
    drop(value);
    output_findings(scan, outputs);
    (projects, app_mode)
}

/// Adds a project unless its path is known already; a later source for the
/// same path only upgrades the trust.
fn add(
    projects: &mut Vec<Project>,
    path: TargetPath,
    trusted: bool,
    origin: Origin,
    source: FindingSource,
) {
    let listed = origin == Origin::Config;
    if let Some(known) = projects.iter_mut().find(|p| p.path.same_path(&path)) {
        known.trusted |= trusted;
        known.listed |= listed;
        return;
    }
    projects.push(Project {
        path,
        trusted,
        listed,
        origin,
        source,
    });
}

/// String values of an object in the app state with their pointers, one
/// per distinct path.
fn string_values(state: &Value, key: &str) -> Vec<(String, String)> {
    let Some(object) = state.get(key).and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut values: Vec<(String, String)> = Vec::new();
    for (id, value) in object.iter().take(MAX_APP_PATHS) {
        let Some(raw) = value.as_str() else { continue };
        if !values.iter().any(|(_, known)| known == raw) {
            values.push((format!("/{key}/{}", pointer_key(id)), raw.to_string()));
        }
    }
    values
}

pub(crate) fn findings(scan: &mut Scan, projects: &[Project], policies: &Policies) {
    for (index, project) in projects.iter().enumerate() {
        let policy = &policies.projects[index];
        let (access, explanation) = match policy.mode {
            Mode::ReadOnly => (AccessKind::Read, "finding.codex.workingDirectory.read"),
            Mode::WorkspaceWrite => (
                AccessKind::ReadWrite,
                "finding.codex.workingDirectory.write",
            ),
            Mode::FullAccess => (AccessKind::ReadWrite, "finding.codex.workingDirectory.full"),
        };
        let (status, confidence, scope) = match project.origin {
            Origin::Config => (
                FindingStatus::Configured,
                Confidence::High,
                FindingScope::User,
            ),
            Origin::App => (
                FindingStatus::Observed,
                Confidence::Medium,
                FindingScope::Project,
            ),
            Origin::Observed => (
                FindingStatus::Observed,
                Confidence::Low,
                FindingScope::Session,
            ),
        };
        let finding = Finding::builder(
            &scan.meta,
            scope,
            FindingTarget::path(
                project.path.to_string(),
                PatternKind::Directory,
                project.path.to_path_buf(),
            ),
            project.source.clone(),
        )
        .effect(RuleEffect::Allow)
        .access(access)
        .status(status)
        .confidence(confidence)
        .explanation(Message::new(explanation).arg("mode", &policy.label))
        .build();
        scan.finding(finding);
    }
}

fn output_findings(scan: &mut Scan, outputs: Vec<OutputDirectory>) {
    for output in outputs {
        let finding = Finding::builder(
            &scan.meta,
            FindingScope::Session,
            FindingTarget::path(
                output.path.to_string(),
                PatternKind::Directory,
                output.path.to_path_buf(),
            ),
            output.source,
        )
        .effect(RuleEffect::Allow)
        .access(AccessKind::Write)
        .status(FindingStatus::Observed)
        .confidence(Confidence::Low)
        .explanation(Message::new("finding.codex.outputDirectory"))
        .build();
        scan.finding(finding);
    }
}
