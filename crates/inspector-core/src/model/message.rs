//! User-facing texts as message keys with parameters.
//!
//! The core never produces finished sentences for the GUI. A [`Message`]
//! carries a stable key, named parameters and the English text rendered from
//! [`CATALOG`]. The CLI and JSON output use the English text; the GUI
//! translates by key and fills in the parameters.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    /// Stable key, e.g. `warning.missingPath`.
    pub key: String,
    /// Named parameters, e.g. paths. They are never translated.
    pub args: BTreeMap<String, String>,
    /// English text, rendered from the catalog.
    pub text: String,
}

impl Message {
    /// A message without parameters. The key must be in [`CATALOG`].
    pub fn new(key: &str) -> Self {
        debug_assert!(template(key).is_some(), "unknown message key '{key}'");
        let mut message = Self {
            key: key.into(),
            args: BTreeMap::new(),
            text: String::new(),
        };
        message.render();
        message
    }

    /// Adds a named parameter that fills `{name}` in the template.
    pub fn arg(mut self, name: &str, value: impl ToString) -> Self {
        self.args.insert(name.into(), value.to_string());
        self.render();
        self
    }

    /// A technical value that is shown as is, e.g. a mode name or a count.
    pub fn raw(value: impl ToString) -> Self {
        Self::new("value.raw").arg("value", value)
    }

    fn render(&mut self) {
        let template = template(&self.key).unwrap_or(&self.key);
        self.text = fill(template, &self.args);
    }
}

fn template(key: &str) -> Option<&'static str> {
    CATALOG
        .binary_search_by(|(k, _)| (*k).cmp(key))
        .ok()
        .map(|i| CATALOG[i].1)
}

/// Replaces `{name}` with the parameter `name`; unknown placeholders stay.
fn fill(template: &str, args: &BTreeMap<String, String>) -> String {
    let mut text = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        text.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('}').map(|end| (&after[..end], end)) {
            Some((name, end)) if args.contains_key(name) => {
                text.push_str(&args[name]);
                rest = &after[end + 1..];
            }
            _ => {
                text.push('{');
                rest = after;
            }
        }
    }
    text.push_str(rest);
    text
}

/// English texts of all messages, sorted by key. This is the source for the
/// GUI's translation files.
pub const CATALOG: &[(&str, &str)] = &[
    (
        "diagnostic.demoScanner",
        "Demo scanner: all values are examples.",
    ),
    (
        "diagnostic.deprecatedLocation",
        "Claude Code no longer reads {path}; its settings have no effect.",
    ),
    (
        "diagnostic.fileTooLarge",
        "The file is larger than {limit} and was not read.",
    ),
    (
        "diagnostic.ineffectiveRule",
        "The rule {rule} never matches: Claude Code does not evaluate path rules for {tool}. Use Read(...) or Edit(...) instead.",
    ),
    (
        "diagnostic.internalScannerError",
        "Scanner '{scanner}' failed unexpectedly: {reason}",
    ),
    (
        "diagnostic.invalidJson",
        "The file is not valid JSON: {error}",
    ),
    (
        "diagnostic.legacyAllowedTools",
        "~/.claude.json lists allowed tools for {project} in an older format; they are not evaluated.",
    ),
    (
        "diagnostic.managedOnlySetting",
        "{key} only takes effect in managed settings and is ignored here.",
    ),
    (
        "diagnostic.mcpManagedExclusive",
        "managed-mcp.json is present; MCP servers from other levels are not used.",
    ),
    (
        "diagnostic.mcpServerDenied",
        "The MCP server '{server}' is blocked by allowedMcpServers or deniedMcpServers and is not evaluated.",
    ),
    (
        "diagnostic.mcpServerDisabled",
        "The MCP server '{server}' is disabled and is not evaluated.",
    ),
    (
        "diagnostic.mcpServerShadowed",
        "The MCP server '{server}' is defined on several levels; only the definition with the highest priority is evaluated.",
    ),
    (
        "diagnostic.mdmProfileDetected",
        "A managed configuration profile exists ({path}); it is not evaluated.",
    ),
    (
        "diagnostic.notARegularFile",
        "Not a regular file; it was not read.",
    ),
    (
        "diagnostic.policyHelper",
        "Managed settings use a policy helper program; its output is not evaluated.",
    ),
    (
        "diagnostic.scopeNotAllowed",
        "{key} = {value} has no effect in project and local settings.",
    ),
    (
        "diagnostic.unexpectedType",
        "Unexpected value at {pointer}: expected {expected}.",
    ),
    (
        "diagnostic.unknownKey",
        "Unknown setting {key}; it was not evaluated.",
    ),
    (
        "diagnostic.unparsableRule",
        "The rule {rule} could not be interpreted.",
    ),
    (
        "diagnostic.unreadableFile",
        "The file could not be read: {error}",
    ),
    (
        "diagnostic.unresolvedPath",
        "The path {path} could not be resolved: {error}",
    ),
    (
        "diagnostic.unresolvedVariable",
        "The variable {variable} is not set; the value is shown unexpanded.",
    ),
    (
        "finding.additionalDirectory",
        "Additional working directory: readable without a prompt, writable depending on the permission mode.",
    ),
    (
        "finding.knownProject",
        "Known project: Claude Code has been used here. It can read files in this directory without a prompt; writing depends on the permission mode.",
    ),
    (
        "finding.mcpFilesystemPath",
        "Directory the filesystem MCP server '{server}' may read and write.",
    ),
    (
        "finding.mcpPathArgument",
        "Path argument of the local MCP server '{server}'.",
    ),
    (
        "finding.mcpServer",
        "Local MCP server '{server}' ({command}): a separate program with your user rights. Claude Code's permission rules and sandbox do not restrict it.",
    ),
    (
        "finding.mode.auto",
        "Permission mode auto: a classifier approves actions without asking.",
    ),
    (
        "finding.mode.bypassPermissions",
        "Permission mode bypassPermissions: Claude Code skips all permission prompts.",
    ),
    ("finding.noExplanation", "No explanation available."),
    (
        "finding.rule.edit.allow",
        "Allow rule: Claude Code may edit these paths without a prompt.",
    ),
    (
        "finding.rule.edit.ask",
        "Ask rule: Claude Code asks before editing these paths.",
    ),
    (
        "finding.rule.edit.deny",
        "Deny rule: Claude Code may not edit these paths.",
    ),
    (
        "finding.rule.negation",
        "Exception: paths matching this pattern are excluded from the preceding {effect} rules.",
    ),
    (
        "finding.rule.read.allow",
        "Allow rule: Claude Code's file tools may read these paths without a prompt.",
    ),
    (
        "finding.rule.read.ask",
        "Ask rule: Claude Code asks before reading these paths.",
    ),
    (
        "finding.rule.read.deny",
        "Deny rule: Claude Code's file tools may not read these paths. Shell commands that name these paths are blocked as well, other programs are not.",
    ),
    (
        "finding.sandbox.allowRead",
        "Sandbox: shell commands may read here again despite a denyRead setting.",
    ),
    (
        "finding.sandbox.allowWrite",
        "Sandbox: shell commands may additionally write here.",
    ),
    (
        "finding.sandbox.denyRead",
        "Sandbox: shell commands may not read here. Claude Code's Read tool is not affected.",
    ),
    (
        "finding.sandbox.denyWrite",
        "Sandbox: shell commands may not write here.",
    ),
    (
        "finding.sandboxedShell",
        "Shell commands run in a sandbox: they can read almost the whole computer, but write only to the working directory, the temporary directory and allowed paths.",
    ),
    (
        "finding.unsandboxedShell",
        "Shell commands run without a sandbox and can reach everything the user can.",
    ),
    ("summary.hooks", "Hooks"),
    ("summary.localMcpServers", "Local MCP servers"),
    ("summary.managedSettings", "Managed settings"),
    ("summary.permissionMode", "Permission mode"),
    ("summary.projects", "Known projects"),
    (
        "summary.readsOutsideWorkingDirectories",
        "File tools outside working directories",
    ),
    ("summary.remoteMcpServers", "Remote MCP servers"),
    ("summary.sandbox", "Sandbox"),
    ("summary.shell", "Shell commands"),
    ("value.allowedWithoutPrompt", "allowed without prompt"),
    ("value.blocked", "blocked"),
    ("value.disabled", "disabled"),
    ("value.enabled", "enabled"),
    ("value.none", "none"),
    ("value.notSupported", "not supported on this system"),
    ("value.raw", "{value}"),
    ("value.withPrompt", "with prompt"),
    (
        "warning.containsHome",
        "Grants {path}, which contains the home directory {home}.",
    ),
    (
        "warning.dangerousMode",
        "Permission mode {mode} lets Claude Code act without asking.",
    ),
    ("warning.filesystemRoot", "Grants the entire filesystem."),
    (
        "warning.homeDirectory",
        "Grants the entire home directory with all personal files.",
    ),
    (
        "warning.likelyMisanchored",
        "{rule} starts with a single slash and is relative to {anchor}, not absolute. Use two slashes for an absolute path.",
    ),
    (
        "warning.managedRulesOnly",
        "Ignored: only permission rules from managed settings apply (allowManagedPermissionRulesOnly).",
    ),
    (
        "warning.mcpPendingApproval",
        "Claude Code asks for approval before starting this server; until then it is not active.",
    ),
    ("warning.missingPath", "The path does not exist (anymore)."),
    (
        "warning.overriddenByDeny",
        "Overridden by deny rule {rule}; deny takes precedence.",
    ),
    (
        "warning.personalFolder",
        "Grants the personal folder {folder}.",
    ),
    (
        "warning.sandboxDisabled",
        "The sandbox is not enabled, so this setting has no effect.",
    ),
    (
        "warning.sandboxExcludedCommand",
        "Commands matching {command} run outside the sandbox.",
    ),
    (
        "warning.sandboxUnsupported",
        "There is no sandbox on native Windows; sandbox.enabled has no effect.",
    ),
    (
        "warning.sensitiveLocation",
        "{location} typically holds credentials.",
    ),
    (
        "warning.shellAllowedWithoutPrompt",
        "The allow rule {rule} lets Claude Code run any shell command without a prompt.",
    ),
    (
        "warning.symlinkElsewhere",
        "Symlink to {target}, outside of {parent}.",
    ),
    (
        "warning.trustNotAccepted",
        "Takes effect only after the project's trust dialog has been accepted.",
    ),
    (
        "warning.unsandboxedRetry",
        "Failed commands may be retried outside the sandbox (sandbox.allowUnsandboxedCommands).",
    ),
    (
        "warning.unsandboxedShell",
        "No sandbox is configured; shell commands run with the user's rights.",
    ),
    ("warning.wholeDrive", "Grants the whole drive {path}."),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_sorted_and_unique() {
        for pair in CATALOG.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} before {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn catalog_keys_and_placeholders_are_well_formed() {
        let identifier =
            |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '.');
        for (key, template) in CATALOG {
            assert!(identifier(key), "bad key {key}");
            let mut rest = *template;
            while let Some(start) = rest.find('{') {
                let end = rest[start..].find('}').expect("unclosed placeholder") + start;
                let name = &rest[start + 1..end];
                assert!(identifier(name), "bad placeholder {{{name}}} in {key}");
                rest = &rest[end + 1..];
            }
        }
    }

    #[test]
    fn renders_parameters() {
        let message = Message::new("warning.symlinkElsewhere")
            .arg("target", "/srv")
            .arg("parent", "/home/alice");
        assert_eq!(message.text, "Symlink to /srv, outside of /home/alice.");
        assert_eq!(message.args["target"], "/srv");
    }

    #[test]
    fn missing_parameters_stay_visible() {
        assert_eq!(
            Message::new("warning.wholeDrive").text,
            "Grants the whole drive {path}."
        );
    }

    #[test]
    fn parameters_are_inserted_literally() {
        let message = Message::raw("{value} and {other}");
        assert_eq!(message.text, "{value} and {other}");
    }

    #[test]
    fn serializes_key_args_and_text() {
        let json = serde_json::to_value(Message::raw("bypassPermissions")).unwrap();
        assert_eq!(json["key"], "value.raw");
        assert_eq!(json["args"]["value"], "bypassPermissions");
        assert_eq!(json["text"], "bypassPermissions");
    }

    #[test]
    #[should_panic(expected = "unknown message key")]
    fn unknown_keys_are_caught_in_debug_builds() {
        let _ = Message::new("no.such.key");
    }
}
