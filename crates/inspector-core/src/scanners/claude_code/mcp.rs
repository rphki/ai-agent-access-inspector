//! MCP servers: separate programs that run with the user's rights.
//!
//! Levels: managed (`managed-mcp.json`, exclusive if present, and
//! `managedMcpServers`), local (`.claude.json` → `projects.<path>`),
//! project (`.mcp.json`) and user (`.claude.json` top level).

use std::collections::BTreeSet;

use serde_json::Value;

use super::projects::Project;
use super::settings::{Level, SettingsFile, View};
use super::{ClaudeJson, Locations, Scan, pointer_key, unexpected_type};
use crate::analysis::path::TargetPath;
use crate::analysis::resolve::{expand_env, resolve_path};
use crate::model::{
    AccessKind, Confidence, Diagnostic, DiagnosticCode, DiagnosticLevel, Finding, FindingScope,
    FindingStatus, FindingSubject, FindingTarget, Message, PatternKind, RuleEffect, Severity,
    SourceDocumentation, SummaryEntry, Warning, WarningKind,
};
use crate::redact::{REDACTED, redact_args};

/// Package name of the reference filesystem server; its path arguments are
/// the directories it may read and write.
const FILESYSTEM_SERVER: &str = "server-filesystem";

/// Longest command line shown in a finding.
const MAX_COMMAND_DISPLAY: usize = 200;

/// Where a server definition comes from.
struct Origin<'a> {
    scope: FindingScope,
    file: &'a TargetPath,
    config_kind: &'static str,
    documentation: SourceDocumentation,
    /// JSON pointer of the `mcpServers` object.
    pointer: String,
    project: Option<&'a Project>,
}

#[derive(Default)]
struct Counts {
    local: usize,
    remote: usize,
}

pub(crate) fn evaluate(
    scan: &mut Scan,
    files: &[SettingsFile],
    projects: &[Project],
    claude_json: Option<&ClaudeJson>,
) {
    let global = View::global(files);
    let policy = Policy::new(scan, &global);
    let mut counts = Counts::default();

    // Remote servers from managed settings never run locally.
    for file in global.files().iter().filter(|f| f.level == Level::Managed) {
        if let Some(servers) = file
            .pointer("/managedMcpServers")
            .and_then(Value::as_object)
        {
            counts.remote += servers.len();
        }
    }

    let managed_mcp = scan.locations.managed_mcp();
    if let Some(value) = scan.read_json(&managed_mcp) {
        scan.diagnostic(
            Diagnostic::new(
                DiagnosticLevel::Info,
                DiagnosticCode::Other,
                Message::new("diagnostic.mcpManagedExclusive"),
            )
            .file(managed_mcp.to_path_buf()),
        );
        let origin = Origin {
            scope: FindingScope::Managed,
            file: &managed_mcp,
            config_kind: "managed-mcp",
            documentation: SourceDocumentation::Documented,
            pointer: "/mcpServers".into(),
            project: None,
        };
        for (name, server) in servers(scan, &value, &origin) {
            evaluate_server(scan, &name, &server, &origin, false, &mut counts);
        }
        push_summary(scan, &counts);
        return;
    }

    let mut user_names = BTreeSet::new();
    if let Some(claude_json) = claude_json
        && let Some(value) = &claude_json.mcp_servers
    {
        let origin = Origin {
            scope: FindingScope::User,
            file: &claude_json.path,
            config_kind: "claude-json",
            documentation: SourceDocumentation::Documented,
            pointer: "/mcpServers".into(),
            project: None,
        };
        let wrapped = serde_json::json!({ "mcpServers": value });
        for (name, server) in servers(scan, &wrapped, &origin) {
            if policy.allows(scan, &name, &server, origin.file) {
                user_names.insert(name.clone());
                evaluate_server(scan, &name, &server, &origin, false, &mut counts);
            }
        }
    }

    for (index, project) in projects.iter().enumerate() {
        let view = View::project(files, index);
        let entry = project.entry.as_ref();
        let disabled_by_toggle = string_set(entry.and_then(|e| e.get("disabledMcpServers")));
        let mut local_names = BTreeSet::new();

        if let (Some(claude_json), Some(entry)) = (claude_json, entry)
            && let Some(value) = entry.get("mcpServers")
        {
            let key = project.key.clone().unwrap_or_default();
            let origin = Origin {
                scope: FindingScope::ProjectLocal,
                file: &claude_json.path,
                config_kind: "claude-json",
                documentation: SourceDocumentation::Documented,
                pointer: format!("/projects/{}/mcpServers", pointer_key(&key)),
                project: Some(project),
            };
            let wrapped = serde_json::json!({ "mcpServers": value });
            for (name, server) in servers(scan, &wrapped, &origin) {
                if disabled_by_toggle.contains(&name) {
                    disabled(scan, &name, origin.file);
                } else if policy.allows(scan, &name, &server, origin.file) {
                    shadow_check(scan, &name, &user_names, origin.file);
                    local_names.insert(name.clone());
                    evaluate_server(scan, &name, &server, &origin, false, &mut counts);
                }
            }
        }

        let mcp_json = Locations::file(&project.path, ".mcp.json");
        if let Some(value) = scan.read_json(&mcp_json) {
            let approval = Approval::new(&view, projects, entry);
            let origin = Origin {
                scope: FindingScope::Project,
                file: &mcp_json,
                config_kind: "mcp-json",
                documentation: SourceDocumentation::Documented,
                pointer: "/mcpServers".into(),
                project: Some(project),
            };
            for (name, server) in servers(scan, &value, &origin) {
                if local_names.contains(&name) {
                    shadow_check(scan, &name, &local_names, origin.file);
                    continue;
                }
                if approval.rejected.contains(&name) || disabled_by_toggle.contains(&name) {
                    disabled(scan, &name, origin.file);
                } else if policy.allows(scan, &name, &server, origin.file) {
                    shadow_check(scan, &name, &user_names, origin.file);
                    let pending = !approval.all && !approval.approved.contains(&name);
                    evaluate_server(scan, &name, &server, &origin, pending, &mut counts);
                }
            }
        }
    }
    push_summary(scan, &counts);
}

/// The server objects of an `mcpServers` map, with type problems reported.
fn servers(scan: &mut Scan, value: &Value, origin: &Origin) -> Vec<(String, Value)> {
    let Some(map) = value.get("mcpServers") else {
        return Vec::new();
    };
    let Some(map) = map.as_object() else {
        scan.diagnostic(unexpected_type(origin.file, &origin.pointer, "an object"));
        return Vec::new();
    };
    map.iter()
        .filter_map(|(name, server)| {
            if server.is_object() {
                Some((name.clone(), server.clone()))
            } else {
                let pointer = format!("{}/{}", origin.pointer, pointer_key(name));
                scan.diagnostic(unexpected_type(origin.file, &pointer, "an object"));
                None
            }
        })
        .collect()
}

fn evaluate_server(
    scan: &mut Scan,
    name: &str,
    server: &Value,
    origin: &Origin,
    pending: bool,
    counts: &mut Counts,
) {
    let pointer = format!("{}/{}", origin.pointer, pointer_key(name));
    let kind = server.get("type").and_then(Value::as_str);
    if kind == Some("sdk") {
        return;
    }
    let remote = matches!(kind, Some("http" | "streamable-http" | "sse" | "ws"))
        || (kind.is_none() && server.get("url").is_some());
    if remote {
        counts.remote += 1;
        return;
    }
    let Some(command) = server.get("command").and_then(Value::as_str) else {
        scan.diagnostic(unexpected_type(
            origin.file,
            &format!("{pointer}/command"),
            "a string",
        ));
        return;
    };
    counts.local += 1;

    let mut args = Vec::new();
    if let Some(values) = server.get("args") {
        match values.as_array() {
            Some(values) => {
                for (i, value) in values.iter().enumerate() {
                    match value.as_str() {
                        Some(arg) => args.push(arg.to_string()),
                        None => scan.diagnostic(unexpected_type(
                            origin.file,
                            &format!("{pointer}/args/{i}"),
                            "a string",
                        )),
                    }
                }
            }
            None => scan.diagnostic(unexpected_type(
                origin.file,
                &format!("{pointer}/args"),
                "an array of strings",
            )),
        }
    }

    let command = expand(scan, command, origin.file, &format!("{pointer}/command"));
    let args: Vec<String> = args
        .iter()
        .enumerate()
        .map(|(i, arg)| expand(scan, arg, origin.file, &format!("{pointer}/args/{i}")))
        .collect();
    // The command itself is redacted too: it can be a wrapper that carries
    // a token, e.g. `env TOKEN=… server`.
    let shown = redact_args(
        &std::iter::once(command.clone())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let shown_args = &shown[1..];
    let display = truncate(&shown.join(" "));
    let filesystem_server =
        command.contains(FILESYSTEM_SERVER) || args.iter().any(|a| a.contains(FILESYSTEM_SERVER));
    let subject = FindingSubject::McpServer { name: name.into() };
    let status = |configured| {
        if pending {
            FindingStatus::Unknown
        } else {
            configured
        }
    };

    let source = scan
        .source(
            origin.config_kind,
            origin.documentation,
            origin.file,
            &pointer,
        )
        .raw_value(&display);
    let mut builder = Finding::builder(
        &scan.meta,
        origin.scope,
        FindingTarget::unresolved(&display, PatternKind::Everything),
        source,
    )
    .subject(subject.clone())
    .effect(RuleEffect::Allow)
    .access(AccessKind::ReadWrite)
    .status(status(FindingStatus::Potential))
    .confidence(Confidence::Medium)
    .explanation(
        Message::new("finding.mcpServer")
            .arg("server", name)
            .arg("command", &display),
    );
    if pending {
        builder = builder.warning(pending_warning());
    }
    scan.finding(builder.build());

    let home = scan.locations.home.clone();
    for (i, arg) in args.iter().enumerate() {
        if shown_args[i] == REDACTED || !is_path_like(arg) {
            continue;
        }
        let Ok(path) = resolve_path(arg, &home, origin.project.map(|p| &p.path)) else {
            continue;
        };
        let source = scan
            .source(
                origin.config_kind,
                origin.documentation,
                origin.file,
                &format!("{pointer}/args/{i}"),
            )
            .raw_value(arg);
        let (kind, access, confidence, explanation) = if filesystem_server {
            (
                PatternKind::Directory,
                AccessKind::ReadWrite,
                Confidence::High,
                "finding.mcpFilesystemPath",
            )
        } else {
            (
                PatternKind::ExactPath,
                AccessKind::Unknown,
                Confidence::Medium,
                "finding.mcpPathArgument",
            )
        };
        let mut builder = Finding::builder(
            &scan.meta,
            origin.scope,
            FindingTarget::path(arg, kind, path.to_path_buf()),
            source,
        )
        .subject(subject.clone())
        .effect(RuleEffect::Allow)
        .access(access)
        .status(status(FindingStatus::Configured))
        .confidence(confidence)
        .explanation(Message::new(explanation).arg("server", name));
        if pending {
            builder = builder.warning(pending_warning());
        }
        scan.finding(builder.build());
    }
}

fn pending_warning() -> Warning {
    Warning::new(
        WarningKind::NotEffective {
            reason: crate::model::IneffectiveReason::ApprovalPending,
        },
        Severity::Info,
        Message::new("warning.mcpPendingApproval"),
    )
}

/// Expands `${VAR}`; unset variables are reported.
fn expand(scan: &mut Scan, value: &str, file: &TargetPath, pointer: &str) -> String {
    let expansion = expand_env(value, &scan.context.env);
    for variable in &expansion.unresolved {
        scan.diagnostic(
            Diagnostic::new(
                DiagnosticLevel::Info,
                DiagnosticCode::UnresolvedVariable,
                Message::new("diagnostic.unresolvedVariable").arg("variable", variable),
            )
            .file(file.to_path_buf())
            .pointer(pointer),
        );
    }
    expansion.value
}

fn is_path_like(arg: &str) -> bool {
    if arg.starts_with('-') {
        return false;
    }
    let mut chars = arg.chars();
    let drive = matches!(
        (chars.next(), chars.next(), chars.next()),
        (Some(letter), Some(':'), Some('\\' | '/')) if letter.is_ascii_alphabetic()
    );
    drive
        || arg.starts_with('/')
        || arg == "~"
        || arg.starts_with("~/")
        || arg.starts_with("~\\")
        || arg.starts_with("./")
        || arg.starts_with("../")
        || arg.starts_with("\\\\")
}

fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_COMMAND_DISPLAY {
        return text.to_string();
    }
    let mut shortened: String = text.chars().take(MAX_COMMAND_DISPLAY).collect();
    shortened.push('…');
    shortened
}

fn disabled(scan: &mut Scan, name: &str, file: &TargetPath) {
    scan.diagnostic(
        Diagnostic::new(
            DiagnosticLevel::Info,
            DiagnosticCode::Other,
            Message::new("diagnostic.mcpServerDisabled").arg("server", name),
        )
        .file(file.to_path_buf()),
    );
}

fn shadow_check(scan: &mut Scan, name: &str, others: &BTreeSet<String>, file: &TargetPath) {
    if others.contains(name) {
        scan.diagnostic(
            Diagnostic::new(
                DiagnosticLevel::Info,
                DiagnosticCode::Other,
                Message::new("diagnostic.mcpServerShadowed").arg("server", name),
            )
            .file(file.to_path_buf()),
        );
    }
}

fn push_summary(scan: &mut Scan, counts: &Counts) {
    for (key, count) in [
        ("localMcpServers", counts.local),
        ("remoteMcpServers", counts.remote),
    ] {
        let value = if count == 0 {
            Message::new("value.none")
        } else {
            Message::raw(count)
        };
        scan.out.summary.push(SummaryEntry::new(
            key,
            Message::new(&format!("summary.{key}")),
            value,
        ));
    }
}

fn string_set(value: Option<&Value>) -> BTreeSet<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

/// Approval state of a project's `.mcp.json` servers.
struct Approval {
    all: bool,
    approved: BTreeSet<String>,
    rejected: BTreeSet<String>,
}

impl Approval {
    fn new(view: &View, projects: &[Project], entry: Option<&Value>) -> Self {
        let mut approval = Self {
            all: false,
            approved: string_set(entry.and_then(|e| e.get("enabledMcpjsonServers"))),
            rejected: string_set(entry.and_then(|e| e.get("disabledMcpjsonServers"))),
        };
        for file in view.files() {
            // Approvals from project files only count in trusted projects;
            // rejections always count.
            let trusted =
                !file.level.is_project() || file.project.is_some_and(|p| projects[p].trusted);
            approval
                .rejected
                .extend(string_set(file.pointer("/disabledMcpjsonServers")));
            if trusted {
                approval.all |=
                    file.pointer("/enableAllProjectMcpServers") == Some(&Value::Bool(true));
                approval
                    .approved
                    .extend(string_set(file.pointer("/enabledMcpjsonServers")));
            }
        }
        approval
    }
}

/// `allowedMcpServers` and `deniedMcpServers` from managed and user settings.
struct Policy {
    allowed: Option<Vec<Value>>,
    denied: Vec<Value>,
}

impl Policy {
    fn new(scan: &mut Scan, global: &View) -> Self {
        let managed_only = global.files().iter().any(|f| {
            f.level == Level::Managed
                && f.pointer("/allowManagedMcpServersOnly") == Some(&Value::Bool(true))
        });
        let mut allowed: Option<Vec<Value>> = None;
        let mut denied = Vec::new();
        for file in global.files() {
            if let Some(list) = file.pointer("/allowedMcpServers") {
                match list.as_array() {
                    Some(list) if !managed_only || file.level == Level::Managed => {
                        allowed
                            .get_or_insert_with(Vec::new)
                            .extend(list.iter().cloned());
                    }
                    Some(_) => {}
                    None => scan.diagnostic(unexpected_type(
                        &file.path,
                        "/allowedMcpServers",
                        "an array",
                    )),
                }
            }
            if let Some(list) = file.pointer("/deniedMcpServers") {
                match list.as_array() {
                    Some(list) => denied.extend(list.iter().cloned()),
                    None => scan.diagnostic(unexpected_type(
                        &file.path,
                        "/deniedMcpServers",
                        "an array",
                    )),
                }
            }
        }
        Self { allowed, denied }
    }

    /// Reports and returns `false` for servers the policy blocks.
    fn allows(&self, scan: &mut Scan, name: &str, server: &Value, file: &TargetPath) -> bool {
        let matches = |entry: &Value| matches_entry(entry, name, server);
        let blocked = self.denied.iter().any(matches)
            || self
                .allowed
                .as_ref()
                .is_some_and(|allowed| !allowed.iter().any(matches));
        if blocked {
            scan.diagnostic(
                Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticCode::Other,
                    Message::new("diagnostic.mcpServerDenied").arg("server", name),
                )
                .file(file.to_path_buf()),
            );
        }
        !blocked
    }
}

/// `{"serverName": …}` or `{"serverCommand": [command, args…]}`. URL entries
/// only concern remote servers, which produce no folder findings.
fn matches_entry(entry: &Value, name: &str, server: &Value) -> bool {
    if let Some(entry_name) = entry.get("serverName").and_then(Value::as_str) {
        return entry_name == name;
    }
    if let Some(entry_command) = entry.get("serverCommand").and_then(Value::as_array) {
        let mut command: Vec<&Value> = server.get("command").into_iter().collect();
        command.extend(
            server
                .get("args")
                .and_then(Value::as_array)
                .into_iter()
                .flatten(),
        );
        return entry_command.iter().collect::<Vec<_>>() == command;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_path_like_arguments() {
        for arg in [
            "/srv",
            "~",
            "~/docs",
            "./data",
            "../x",
            r"C:\Users",
            "D:/x",
            r"\\nas\share",
        ] {
            assert!(is_path_like(arg), "{arg}");
        }
        for arg in [
            "-y",
            "--root=/x",
            "server.py",
            "@scope/pkg",
            "https://x",
            "C:",
        ] {
            assert!(!is_path_like(arg), "{arg}");
        }
    }

    #[test]
    fn matches_policy_entries() {
        let server = serde_json::json!({ "command": "npx", "args": ["-y", "pkg"] });
        assert!(matches_entry(
            &serde_json::json!({ "serverName": "files" }),
            "files",
            &server
        ));
        assert!(!matches_entry(
            &serde_json::json!({ "serverName": "other" }),
            "files",
            &server
        ));
        assert!(matches_entry(
            &serde_json::json!({ "serverCommand": ["npx", "-y", "pkg"] }),
            "files",
            &server
        ));
        assert!(!matches_entry(
            &serde_json::json!({ "serverCommand": ["npx"] }),
            "files",
            &server
        ));
    }

    #[test]
    fn truncates_long_commands() {
        let long = "x".repeat(300);
        assert_eq!(truncate(&long).chars().count(), MAX_COMMAND_DISPLAY + 1);
    }
}
