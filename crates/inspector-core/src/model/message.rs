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
        "diagnostic.internalScannerError",
        "Scanner '{scanner}' failed unexpectedly: {reason}",
    ),
    (
        "finding.additionalDirectory",
        "Additional working directory: readable without a prompt, writable depending on the permission mode.",
    ),
    (
        "finding.denyRead",
        "Deny rule: the agent's file tools may not read this path.",
    ),
    (
        "finding.mcpPathArgument",
        "Path argument of the local MCP server '{server}'.",
    ),
    ("finding.noExplanation", "No explanation available."),
    (
        "finding.unsandboxedShell",
        "Shell commands run without a sandbox and can reach everything the user can.",
    ),
    ("summary.permissionMode", "Permission mode"),
    ("summary.sandbox", "Sandbox"),
    ("value.disabled", "disabled"),
    ("value.enabled", "enabled"),
    ("value.raw", "{value}"),
    (
        "warning.containsHome",
        "Grants {path}, which contains the home directory {home}.",
    ),
    ("warning.filesystemRoot", "Grants the entire filesystem."),
    (
        "warning.homeDirectory",
        "Grants the entire home directory with all personal files.",
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
        "warning.sensitiveLocation",
        "{location} typically holds credentials.",
    ),
    (
        "warning.symlinkElsewhere",
        "Symlink to {target}, outside of {parent}.",
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
