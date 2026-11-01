//! A scanner that returns fixed example findings.
//!
//! It reads nothing from disk. It exists to exercise the whole pipeline
//! (CLI, export, later the GUI) independently of any real agent, and only
//! runs when selected explicitly with the id `demo`.

use crate::context::ScanContext;
use crate::model::{
    AccessKind, Confidence, DetectionResult, DetectionStatus, Diagnostic, DiagnosticCode,
    DiagnosticLevel, Finding, FindingScope, FindingSource, FindingStatus, FindingSubject,
    FindingTarget, Message, PathAnchor, PatternKind, RuleEffect, ScanOutput, ScannerMetadata,
    Severity, SourceDocumentation, SummaryEntry, Warning, WarningKind,
};
use crate::scanner::AgentScanner;

const PARSER_VERSION: &str = "1";
const CONFIG_KIND: &str = "demo/settings";

pub struct DemoScanner;

impl AgentScanner for DemoScanner {
    fn metadata(&self) -> ScannerMetadata {
        ScannerMetadata {
            id: "demo".into(),
            name: "Demo".into(),
            description: "Fixed example findings for testing and development".into(),
            parser_version: PARSER_VERSION.into(),
        }
    }

    fn detect(&self, _context: &ScanContext) -> DetectionResult {
        DetectionResult {
            status: DetectionStatus::Configured,
            evidence: Vec::new(),
            version: None,
        }
    }

    fn scan(&self, context: &ScanContext) -> ScanOutput {
        let meta = self.metadata();
        let home = &context.home_dir;
        let settings = home.join(".demo/settings.json");
        let source = |pointer: &str, raw: &str| {
            FindingSource::new(CONFIG_KIND, SourceDocumentation::Documented, PARSER_VERSION)
                .file(&settings)
                .pointer(pointer)
                .raw_value(raw)
        };

        let projects = Finding::builder(
            &meta,
            FindingScope::User,
            FindingTarget::path("~/projects", PatternKind::Directory, home.join("projects")),
            source("/permissions/additionalDirectories/0", "~/projects"),
        )
        .effect(RuleEffect::Allow)
        .access(AccessKind::ReadWrite)
        .status(FindingStatus::Configured)
        .confidence(Confidence::High)
        .explanation(Message::new("finding.additionalDirectory"))
        .build();

        let ssh = Finding::builder(
            &meta,
            FindingScope::User,
            FindingTarget::path("Read(~/.ssh/**)", PatternKind::Glob, home.join(".ssh")),
            source("/permissions/deny/0", "Read(~/.ssh/**)").anchor(PathAnchor::Home),
        )
        .effect(RuleEffect::Deny)
        .access(AccessKind::Read)
        .status(FindingStatus::Configured)
        .confidence(Confidence::High)
        .explanation(Message::new("finding.denyRead"))
        .build();

        let shell = Finding::builder(
            &meta,
            FindingScope::OperatingSystem,
            FindingTarget::unresolved("Bash", PatternKind::Everything),
            FindingSource::new(CONFIG_KIND, SourceDocumentation::Derived, PARSER_VERSION),
        )
        .effect(RuleEffect::Allow)
        .access(AccessKind::ReadWrite)
        .status(FindingStatus::Potential)
        .confidence(Confidence::Medium)
        .explanation(Message::new("finding.unsandboxedShell"))
        .warning(Warning::new(
            WarningKind::UnsandboxedShell,
            Severity::High,
            Message::new("warning.unsandboxedShell"),
        ))
        .build();

        let filesystem_server = Finding::builder(
            &meta,
            FindingScope::Project,
            FindingTarget::path("/srv/data", PatternKind::Directory, "/srv/data".into()),
            FindingSource::new("demo/mcp", SourceDocumentation::Documented, PARSER_VERSION)
                .file(home.join("projects/app/.mcp.json"))
                .pointer("/mcpServers/files/args/2")
                .raw_value("/srv/data"),
        )
        .subject(FindingSubject::McpServer {
            name: "files".into(),
        })
        .effect(RuleEffect::Allow)
        .access(AccessKind::ReadWrite)
        .status(FindingStatus::Configured)
        .confidence(Confidence::Medium)
        .explanation(Message::new("finding.mcpPathArgument").arg("server", "files"))
        .build();

        ScanOutput {
            findings: vec![projects, ssh, shell, filesystem_server],
            diagnostics: vec![Diagnostic::new(
                DiagnosticLevel::Info,
                DiagnosticCode::Other,
                Message::new("diagnostic.demoScanner"),
            )],
            summary: vec![
                SummaryEntry::new(
                    "permissionMode",
                    Message::new("summary.permissionMode"),
                    Message::raw("default"),
                ),
                SummaryEntry::new(
                    "sandbox",
                    Message::new("summary.sandbox"),
                    Message::new("value.disabled"),
                ),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::context::EnvSnapshot;
    use crate::platform::OperatingSystem;

    #[test]
    fn findings_are_rooted_in_the_context_home() {
        let context = ScanContext {
            home_dir: PathBuf::from("/home/alice"),
            current_dir: None,
            operating_system: OperatingSystem::Linux,
            env: EnvSnapshot::default(),
            system_root: None,
        };
        let output = DemoScanner.scan(&context);
        assert_eq!(output.findings.len(), 4);
        assert_eq!(
            output.findings[0].target.base_path,
            Some(PathBuf::from("/home/alice/projects"))
        );
        let ids: Vec<&str> = output.findings.iter().map(|f| f.id.as_str()).collect();
        assert!(ids.iter().all(|id| id.starts_with("demo-")));
    }
}
