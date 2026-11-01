//! Markdown report of a scan. English by default (the CLI's language); the
//! GUI passes the translation table of its chosen language.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use crate::model::{
    CATALOG, DetectionStatus, DiagnosticLevel, Existence, Finding, FindingStatus, FindingSubject,
    Message, ScanReport, ScanResult, fill,
};

/// Options for the Markdown report.
#[derive(Debug, Clone, Copy, Default)]
pub struct MarkdownOptions<'a> {
    /// Include informational diagnostics; errors and warnings are always shown.
    pub verbose: bool,
    /// Texts by message key in the report's language; keys that are missing
    /// fall back to the English catalog.
    pub translations: Option<&'a HashMap<String, String>>,
}

/// Looks up report texts and messages: translation table, then the
/// English catalog, then the message's own text.
#[derive(Clone, Copy, Default)]
struct Texts<'a> {
    translations: Option<&'a HashMap<String, String>>,
}

impl Texts<'_> {
    fn template(&self, key: &str) -> Option<&str> {
        self.translations
            .and_then(|t| t.get(key).map(String::as_str))
            .or_else(|| {
                CATALOG
                    .binary_search_by(|(k, _)| (*k).cmp(key))
                    .ok()
                    .map(|i| CATALOG[i].1)
            })
    }

    /// A fixed report text with parameters.
    fn get(&self, key: &str, args: &[(&str, &str)]) -> String {
        let args: BTreeMap<String, String> = args
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        fill(self.template(key).unwrap_or(key), &args)
    }

    fn message(&self, message: &Message) -> String {
        match self.template(&message.key) {
            Some(template) => fill(template, &message.args),
            None => message.text.clone(),
        }
    }

    /// The label of an enum value, e.g. `report.scope.projectLocal`.
    fn label<T: serde::Serialize>(&self, group: &str, value: &T) -> String {
        let name = enum_name(value);
        let key = format!("report.{group}.{name}");
        match self.template(&key) {
            Some(template) => template.to_string(),
            None => label(&name),
        }
    }

    /// `1 finding`, `2 findings`.
    fn count(&self, n: usize, noun: &str) -> String {
        let form = if n == 1 { "one" } else { "other" };
        self.get(
            &format!("report.count.{noun}.{form}"),
            &[("count", &n.to_string())],
        )
    }
}

pub fn render(report: &ScanReport, options: MarkdownOptions) -> String {
    let t = Texts {
        translations: options.translations,
    };
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", t.get("report.title", &[]));
    let _ = writeln!(
        out,
        "{}\n",
        t.get(
            "report.generated",
            &[
                ("tool", &report.tool.name),
                ("version", &report.tool.version),
                ("os", os_name(report)),
                ("format", &report.format_version),
            ]
        )
    );
    if report.results.is_empty() {
        let _ = writeln!(out, "{}", t.get("report.noScanner", &[]));
    }
    for result in &report.results {
        render_result(&mut out, result, options, t);
    }
    out
}

fn os_name(report: &ScanReport) -> &'static str {
    match report.operating_system {
        crate::platform::OperatingSystem::MacOs => "macOS",
        crate::platform::OperatingSystem::Linux => "Linux",
        crate::platform::OperatingSystem::Windows => "Windows",
    }
}

fn render_result(out: &mut String, result: &ScanResult, options: MarkdownOptions, t: Texts) {
    let _ = writeln!(out, "## {}\n", result.scanner.name);
    if result.detection.status == DetectionStatus::NotDetected {
        let _ = writeln!(out, "{}\n", t.get("report.notDetected", &[]));
        render_diagnostics(out, result, options, t);
        return;
    }
    let detected = match &result.detection.version {
        Some(version) => t.get("report.detectedVersion", &[("version", version)]),
        None => t.get("report.detected", &[]),
    };
    let _ = writeln!(out, "{detected}\n");

    if !result.summary.is_empty() {
        let _ = writeln!(
            out,
            "### {}\n\n| {} | {} |\n|---|---|",
            t.get("report.heading.summary", &[]),
            t.get("report.column.setting", &[]),
            t.get("report.column.value", &[]),
        );
        for entry in &result.summary {
            let _ = writeln!(
                out,
                "| {} | {} |",
                cell(&t.message(&entry.label)),
                cell(&t.message(&entry.value))
            );
        }
        out.push('\n');
    }

    // Observed paths (e.g. known projects) say "the agent has been here";
    // they are listed compactly after the rules and grants.
    let (observed, findings): (Vec<&Finding>, Vec<&Finding>) = result
        .findings
        .iter()
        .partition(|f| f.status == FindingStatus::Observed && f.warnings.is_empty());

    let warnings: usize = findings.iter().map(|f| f.warnings.len()).sum();
    let _ = writeln!(
        out,
        "### {}\n\n{}\n",
        t.get("report.heading.findings", &[]),
        t.get(
            "report.totals",
            &[
                ("findings", &t.count(findings.len(), "finding")),
                ("warnings", &t.count(warnings, "warning")),
            ]
        )
    );
    if !findings.is_empty() {
        let columns = [
            "path",
            "holder",
            "effect",
            "access",
            "status",
            "scope",
            "explanation",
            "warnings",
        ];
        for column in columns {
            let _ = write!(out, "| {} ", t.get(&format!("report.column.{column}"), &[]));
        }
        out.push_str("|\n|---|---|---|---|---|---|---|---|\n");
        for finding in by_severity(findings) {
            render_finding(out, finding, t);
        }
        out.push('\n');
    }
    render_observed(out, &observed, t);
    render_diagnostics(out, result, options, t);
}

/// Most severe warning first; findings without warnings keep their order
/// at the end.
fn by_severity(mut findings: Vec<&Finding>) -> Vec<&Finding> {
    findings.sort_by_key(|f| {
        (
            std::cmp::Reverse(f.warnings.iter().map(|w| w.severity).max()),
            std::cmp::Reverse(f.warnings.len()),
        )
    });
    findings
}

/// Observed paths grouped by explanation, which is printed once per group.
fn render_observed(out: &mut String, observed: &[&Finding], t: Texts) {
    if observed.is_empty() {
        return;
    }
    let _ = writeln!(
        out,
        "### {}\n",
        t.get(
            "report.heading.observed",
            &[("count", &observed.len().to_string())]
        )
    );
    let mut groups: Vec<(&Message, Vec<&Finding>)> = Vec::new();
    for finding in observed {
        match groups
            .iter_mut()
            .find(|(m, _)| m.key == finding.explanation.key)
        {
            Some((_, members)) => members.push(finding),
            None => groups.push((&finding.explanation, vec![finding])),
        }
    }
    for (explanation, members) in groups {
        let _ = writeln!(out, "{}\n", text(&t.message(explanation)));
        for finding in members {
            let note = match finding.path_state.existence {
                Existence::Missing => format!(" {}", t.get("report.noLongerExists", &[])),
                _ => String::new(),
            };
            let _ = writeln!(
                out,
                "- `{}`{note}",
                finding.target.display_path.replace('`', "'")
            );
        }
        out.push('\n');
    }
}

fn render_finding(out: &mut String, finding: &Finding, t: Texts) {
    let holder = match &finding.subject {
        FindingSubject::Agent => finding.agent_name.clone(),
        FindingSubject::McpServer { name } => t.get("report.mcpServer", &[("name", name)]),
    };
    let mut path = format!("`{}`", finding.target.display_path.replace('`', "'"));
    if finding.target.raw != finding.target.display_path {
        let raw = format!("`{}`", finding.target.raw.replace('`', "'"));
        let _ = write!(path, "<br>{}", t.get("report.from", &[("raw", &raw)]));
    }
    let warnings: Vec<String> = finding
        .warnings
        .iter()
        .map(|w| {
            format!(
                "{}: {}",
                t.label("severity", &w.severity),
                t.message(&w.message)
            )
        })
        .collect();
    let _ = writeln!(
        out,
        "| {} | {} | {} | {} | {} | {} | {} | {} |",
        cell(&path),
        cell(&holder),
        t.label("effect", &finding.effect),
        t.label("access", &finding.access),
        t.label("status", &finding.status),
        t.label("scope", &finding.scope),
        cell(&t.message(&finding.explanation)),
        cell(&warnings.join("<br>")),
    );
}

fn render_diagnostics(out: &mut String, result: &ScanResult, options: MarkdownOptions, t: Texts) {
    let shown: Vec<_> = result
        .diagnostics
        .iter()
        .filter(|d| options.verbose || d.level != DiagnosticLevel::Info)
        .collect();
    let hidden = result.diagnostics.len() - shown.len();
    if shown.is_empty() && hidden == 0 {
        return;
    }
    let _ = writeln!(out, "### {}\n", t.get("report.heading.diagnostics", &[]));
    for diagnostic in shown {
        let location = match (&diagnostic.file, &diagnostic.pointer) {
            (Some(file), Some(pointer)) => format!(" (`{}`, `{pointer}`)", file.display()),
            (Some(file), None) => format!(" (`{}`)", file.display()),
            _ => String::new(),
        };
        let _ = writeln!(
            out,
            "- **{}:** {}{location}",
            t.label("level", &diagnostic.level),
            text(&t.message(&diagnostic.message))
        );
    }
    if hidden > 0 {
        let _ = writeln!(
            out,
            "- {}",
            t.get(
                "report.hiddenDiagnostics",
                &[("diagnostics", &t.count(hidden, "hiddenDiagnostic"))]
            )
        );
    }
    out.push('\n');
}

fn text(message: &str) -> String {
    message.replace('\n', " ")
}

/// The serialized (camelCase) name of an enum value.
fn enum_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_default()
}

/// `projectLocal` → `project local`, `High` → `high`.
fn label(name: &str) -> String {
    let mut label = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                label.push(' ');
            }
            label.push(c.to_ascii_lowercase());
        } else {
            label.push(c);
        }
    }
    label
}

/// Escapes a value for a Markdown table cell.
fn cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::context::{EnvSnapshot, ScanContext};
    use crate::model::{
        FindingScope, FindingSource, FindingTarget, PatternKind, Severity, SourceDocumentation,
        Warning, WarningKind,
    };
    use crate::platform::OperatingSystem;
    use crate::scanner::{find_scanner, run_scan};

    fn demo_report() -> ScanReport {
        let context = ScanContext {
            home_dir: PathBuf::from("/nonexistent-home/alice"),
            current_dir: None,
            operating_system: OperatingSystem::Linux,
            env: EnvSnapshot::default(),
            system_root: None,
        };
        run_scan(&context, &[find_scanner("demo").unwrap()])
    }

    #[test]
    fn renders_sections_and_findings() {
        let markdown = render(&demo_report(), MarkdownOptions::default());
        assert!(markdown.starts_with("# AI Agent Access Inspector report"));
        assert!(markdown.contains("on Linux"));
        assert!(markdown.contains("## Demo"));
        assert!(markdown.contains("| Permission mode | default |"));
        // The demo builds paths with the host's separator, so only check the raw value.
        assert!(markdown.contains("<br>from `~/projects`"));
        assert!(markdown.contains("| MCP server files |"));
        assert!(markdown.contains("| allow | read write | configured | user |"));
        assert!(markdown.contains("high: No sandbox is configured"));
    }

    #[test]
    fn info_diagnostics_only_in_verbose_mode() {
        let report = demo_report();
        let normal = render(&report, MarkdownOptions::default());
        assert!(!normal.contains("Demo scanner: all values are examples."));
        assert!(normal.contains("1 informational diagnostic hidden"));
        let verbose = render(
            &report,
            MarkdownOptions {
                verbose: true,
                ..Default::default()
            },
        );
        assert!(verbose.contains("- **info:** Demo scanner: all values are examples."));
    }

    fn finding(raw: &str, status: FindingStatus, warnings: &[Severity]) -> Finding {
        let meta = find_scanner("demo").unwrap().metadata();
        let mut builder = Finding::builder(
            &meta,
            FindingScope::User,
            FindingTarget::unresolved(raw, PatternKind::ExactPath),
            FindingSource::new("test", SourceDocumentation::Derived, "1"),
        )
        .status(status)
        .explanation(Message::new("finding.knownProject"));
        for severity in warnings {
            builder = builder.warning(Warning::new(
                WarningKind::BroadPath,
                *severity,
                Message::new("warning.filesystemRoot"),
            ));
        }
        builder.build()
    }

    #[test]
    fn sorts_findings_by_severity() {
        let a = finding("a", FindingStatus::Configured, &[]);
        let b = finding("b", FindingStatus::Configured, &[Severity::Low]);
        let c = finding("c", FindingStatus::Configured, &[Severity::High]);
        let d = finding(
            "d",
            FindingStatus::Configured,
            &[Severity::Low, Severity::Low],
        );
        let sorted: Vec<&str> = by_severity(vec![&a, &b, &c, &d])
            .iter()
            .map(|f| f.target.raw.as_str())
            .collect();
        assert_eq!(sorted, ["c", "d", "b", "a"]);
    }

    #[test]
    fn lists_observed_paths_compactly() {
        let mut gone = finding("/old", FindingStatus::Observed, &[]);
        gone.path_state.existence = Existence::Missing;
        let here = finding("/here", FindingStatus::Observed, &[]);
        let mut out = String::new();
        render_observed(&mut out, &[&gone, &here], Texts::default());
        assert_eq!(out.matches("Known project: Claude Code").count(), 1);
        assert!(out.contains("- `/old` (no longer exists)"));
        assert!(out.contains("- `/here`\n"));
    }

    #[test]
    fn escapes_table_cells() {
        assert_eq!(cell("a|b\nc"), "a\\|b c");
        assert_eq!(label("projectLocal"), "project local");
        let t = Texts::default();
        assert_eq!(t.count(1, "finding"), "1 finding");
        assert_eq!(t.count(0, "warning"), "0 warnings");
    }

    #[test]
    fn translates_texts_and_messages() {
        let german: HashMap<String, String> = [
            ("report.heading.findings", "Befunde"),
            ("report.count.finding.other", "{count} Befunde"),
            ("report.count.warning.other", "{count} Warnungen"),
            ("report.mcpServer", "MCP-Server {name}"),
            ("report.scope.user", "Benutzer"),
            ("summary.permissionMode", "Berechtigungsmodus"),
            ("warning.unsandboxedShell", "Keine Sandbox konfiguriert."),
            ("diagnostic.demoScanner", "Demo: {missing}"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
        let markdown = render(
            &demo_report(),
            MarkdownOptions {
                verbose: true,
                translations: Some(&german),
            },
        );
        assert!(markdown.contains("### Befunde\n"));
        assert!(markdown.contains(" Befunde, "));
        assert!(markdown.contains("| MCP-Server files |"));
        assert!(markdown.contains("| Benutzer |"));
        assert!(markdown.contains("| Berechtigungsmodus | default |"));
        assert!(markdown.contains("high: Keine Sandbox konfiguriert."));
        // Untranslated keys fall back to English.
        assert!(markdown.contains("### Diagnostics"));
        assert!(markdown.contains("Demo: {missing}"));
    }

    #[test]
    fn translates_messages_with_parameters() {
        let german: HashMap<String, String> = [(
            "warning.wholeDrive".to_string(),
            "Gewährt das gesamte Laufwerk {path}.".to_string(),
        )]
        .into_iter()
        .collect();
        let t = Texts {
            translations: Some(&german),
        };
        let message = Message::new("warning.wholeDrive").arg("path", "D:\\");
        assert_eq!(t.message(&message), "Gewährt das gesamte Laufwerk D:\\.");
        // Unknown keys keep the message's own text.
        let mut unknown = Message::raw("x");
        unknown.key = "warning.fromTheFuture".into();
        unknown.text = "A newer core text.".into();
        assert_eq!(t.message(&unknown), "A newer core text.");
    }
}
