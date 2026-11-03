//! MCP servers and plugins.
//!
//! Stdio MCP servers are started as plain child processes with the user's
//! rights; Codex's sandbox does not confine them. Only `command`, `args`,
//! `url` and `enabled` are read: `env`, `env_vars`, headers and tokens are
//! never touched.

use std::collections::BTreeSet;

use serde_json::Value;

use super::{Layer, Requirements, Scan, View, pointer_key, unexpected_type};
use crate::analysis::resolve::resolve_path;
use crate::model::{
    AccessKind, Confidence, Diagnostic, DiagnosticCode, DiagnosticLevel, Finding, FindingStatus,
    FindingSubject, FindingTarget, Message, PatternKind, RuleEffect, SourceDocumentation,
    SummaryEntry,
};
use crate::redact::{REDACTED, redact_args};
use crate::scanners::claude_code::mcp::{FILESYSTEM_SERVER, is_path_like, truncate};

/// Plugins that let Codex operate other applications or the browser.
const BROAD_PLUGINS: &[(&str, &str)] = &[
    ("computer-use", "computer_use"),
    ("unified-computer-use", "computer_use"),
    ("browser", "browser_use"),
];

#[derive(Default)]
struct Counts {
    local: usize,
    remote: usize,
}

pub(crate) fn evaluate(scan: &mut Scan, layers: &[Layer], requirements: Option<&Requirements>) {
    let allow_list = requirements.and_then(|r| {
        r.value
            .get("mcp_servers")
            .and_then(Value::as_object)
            .map(|servers| servers.keys().cloned().collect::<BTreeSet<_>>())
    });
    let mut counts = Counts::default();
    let mut global_names = BTreeSet::new();
    // Highest layer first: the first definition of a name wins.
    for layer in View::global(layers).layers() {
        for (name, server) in servers(scan, layer) {
            if !global_names.insert(name.clone()) {
                continue;
            }
            evaluate_server(
                scan,
                layer,
                &name,
                &server,
                allow_list.as_ref(),
                &mut counts,
            );
        }
    }
    for layer in layers.iter().filter(|l| l.project.is_some()) {
        for (name, server) in servers(scan, layer) {
            if global_names.contains(&name) {
                scan.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Info,
                        DiagnosticCode::Other,
                        Message::new("diagnostic.mcpServerShadowed").arg("server", &name),
                    )
                    .file(layer.path.to_path_buf()),
                );
            }
            evaluate_server(
                scan,
                layer,
                &name,
                &server,
                allow_list.as_ref(),
                &mut counts,
            );
        }
    }
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

fn servers(scan: &mut Scan, layer: &Layer) -> Vec<(String, Value)> {
    match layer.value.get("mcp_servers") {
        None => Vec::new(),
        Some(Value::Object(servers)) => servers
            .iter()
            .map(|(name, server)| (name.clone(), server.clone()))
            .collect(),
        Some(_) => {
            scan.diagnostic(unexpected_type(&layer.path, "/mcp_servers", "a table"));
            Vec::new()
        }
    }
}

fn evaluate_server(
    scan: &mut Scan,
    layer: &Layer,
    name: &str,
    server: &Value,
    allow_list: Option<&BTreeSet<String>>,
    counts: &mut Counts,
) {
    let pointer = format!("/mcp_servers/{}", pointer_key(name));
    let note = |key: &str| {
        Diagnostic::new(
            DiagnosticLevel::Info,
            DiagnosticCode::Other,
            Message::new(key).arg("server", name),
        )
        .file(layer.path.to_path_buf())
        .pointer(&pointer)
    };
    if server.get("enabled").and_then(Value::as_bool) == Some(false) {
        scan.diagnostic(note("diagnostic.mcpServerDisabled"));
        return;
    }
    if allow_list.is_some_and(|names| !names.contains(name)) {
        scan.diagnostic(note("diagnostic.codex.mcpServerNotAllowed"));
        return;
    }
    if server.get("url").is_some() {
        counts.remote += 1;
        return;
    }
    let Some(command) = server.get("command").and_then(Value::as_str) else {
        scan.diagnostic(unexpected_type(
            &layer.path,
            &format!("{pointer}/command"),
            "a string",
        ));
        return;
    };
    counts.local += 1;
    let mut args = Vec::new();
    match server.get("args") {
        None => {}
        Some(Value::Array(values)) => {
            for (i, value) in values.iter().enumerate() {
                match value.as_str() {
                    Some(arg) => args.push(arg.to_string()),
                    None => scan.diagnostic(unexpected_type(
                        &layer.path,
                        &format!("{pointer}/args/{i}"),
                        "a string",
                    )),
                }
            }
        }
        Some(_) => scan.diagnostic(unexpected_type(
            &layer.path,
            &format!("{pointer}/args"),
            "an array of strings",
        )),
    }

    // The command is redacted too: it can be a wrapper carrying a token.
    let shown = redact_args(
        &std::iter::once(command.to_string())
            .chain(args.iter().cloned())
            .collect::<Vec<_>>(),
    );
    let shown_args = &shown[1..];
    let display = truncate(&shown.join(" "));
    let filesystem_server =
        command.contains(FILESYSTEM_SERVER) || args.iter().any(|a| a.contains(FILESYSTEM_SERVER));
    let subject = FindingSubject::McpServer { name: name.into() };
    let scope = layer.level.scope();
    let config_kind = layer.level.config_kind();

    let source = scan
        .source(
            config_kind,
            SourceDocumentation::Documented,
            &layer.path,
            &pointer,
        )
        .raw_value(&display);
    let finding = Finding::builder(
        &scan.meta,
        scope,
        FindingTarget::unresolved(&display, PatternKind::Everything),
        source,
    )
    .subject(subject.clone())
    .effect(RuleEffect::Allow)
    .access(AccessKind::ReadWrite)
    .status(FindingStatus::Potential)
    .confidence(Confidence::High)
    .explanation(
        Message::new("finding.codex.mcpServer")
            .arg("server", name)
            .arg("command", &display),
    )
    .build();
    scan.finding(finding);

    // Relative arguments are relative to the server's working directory,
    // `cwd` if set.
    let cwd = server
        .get("cwd")
        .and_then(Value::as_str)
        .and_then(|cwd| scan.target_path(cwd));
    let home = scan.locations.home.clone();
    for (i, arg) in args.iter().enumerate() {
        if shown_args[i] == REDACTED || !is_path_like(arg) {
            continue;
        }
        let Ok(path) = resolve_path(arg, &home, cwd.as_ref()) else {
            continue;
        };
        let source = scan
            .source(
                config_kind,
                SourceDocumentation::Documented,
                &layer.path,
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
        let finding = Finding::builder(
            &scan.meta,
            scope,
            FindingTarget::path(arg, kind, path.to_path_buf()),
            source,
        )
        .subject(subject.clone())
        .effect(RuleEffect::Allow)
        .access(access)
        .status(FindingStatus::Configured)
        .confidence(confidence)
        .explanation(Message::new(explanation).arg("server", name))
        .build();
        scan.finding(finding);
    }
}

/// Enabled plugins: a hint for the broad ones, a count for all.
pub(crate) fn plugins(scan: &mut Scan, global: &View, requirements: Option<&Requirements>) {
    let mut seen = BTreeSet::new();
    let mut enabled: Vec<(String, &Layer)> = Vec::new();
    for layer in global.layers() {
        let Some(plugins) = layer.value.get("plugins").and_then(Value::as_object) else {
            continue;
        };
        for (name, plugin) in plugins {
            if name == "_default" || !seen.insert(name.clone()) {
                continue;
            }
            if plugin.get("enabled").and_then(Value::as_bool) == Some(true) {
                enabled.push((name.clone(), layer));
            }
        }
    }
    let feature_off = |feature: &str| {
        requirements
            .and_then(|r| r.value.pointer(&format!("/features/{feature}")))
            .and_then(Value::as_bool)
            == Some(false)
    };
    for (name, layer) in &enabled {
        let short = name.split('@').next().unwrap_or(name);
        let Some((_, feature)) = BROAD_PLUGINS.iter().find(|(plugin, _)| *plugin == short) else {
            continue;
        };
        if feature_off(feature) {
            continue;
        }
        let pointer = format!("/plugins/{}/enabled", pointer_key(name));
        let source = scan
            .source(
                layer.level.config_kind(),
                SourceDocumentation::Heuristic,
                &layer.path,
                &pointer,
            )
            .raw_value(name);
        let finding = Finding::builder(
            &scan.meta,
            layer.level.scope(),
            FindingTarget::unresolved(format!("plugin: {name}"), PatternKind::Everything),
            source,
        )
        .effect(RuleEffect::NotApplicable)
        .access(AccessKind::Unknown)
        .status(FindingStatus::Potential)
        .confidence(Confidence::Low)
        .explanation(Message::new("finding.codex.plugin").arg("plugin", name))
        .build();
        scan.finding(finding);
    }
    let value = if enabled.is_empty() {
        Message::new("value.none")
    } else {
        Message::raw(enabled.len())
    };
    scan.out.summary.push(SummaryEntry::new(
        "plugins",
        Message::new("summary.codex.plugins"),
        value,
    ));
}
