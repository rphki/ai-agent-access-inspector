//! The scanner interface and the scan runner.

use std::panic::{self, AssertUnwindSafe};

use crate::analysis;
use crate::context::ScanContext;
use crate::model::{
    DetectionResult, DetectionStatus, Diagnostic, DiagnosticCode, DiagnosticLevel, Message,
    REPORT_FORMAT_VERSION, ScanOutput, ScanReport, ScanResult, ScannerMetadata, ToolInfo,
};
use crate::scanners;

/// One scanner per agent.
///
/// A scanner detects the agent, locates and reads its configuration
/// tolerantly and translates it into findings. It does not normalise paths
/// globally, deduplicate across scanners, export or change anything.
/// Problems are reported as diagnostics; a scanner never fails as a whole.
pub trait AgentScanner: Send + Sync {
    fn metadata(&self) -> ScannerMetadata;
    fn detect(&self, context: &ScanContext) -> DetectionResult;
    fn scan(&self, context: &ScanContext) -> ScanOutput;
}

/// Runs the given scanners and collects their results into a report.
///
/// `scan` is only called for detected agents. Each scanner's findings then
/// go through the path analysis. A panicking scanner yields an internal
/// diagnostic instead of aborting the whole run.
pub fn run_scan(context: &ScanContext, scanners: &[&dyn AgentScanner]) -> ScanReport {
    ScanReport {
        format_version: REPORT_FORMAT_VERSION.into(),
        tool: ToolInfo::current(),
        operating_system: context.operating_system,
        results: scanners
            .iter()
            .map(|scanner| run_one(context, *scanner))
            .collect(),
    }
}

fn run_one(context: &ScanContext, scanner: &dyn AgentScanner) -> ScanResult {
    let metadata = scanner.metadata();
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| {
        let detection = scanner.detect(context);
        let output = match detection.status {
            DetectionStatus::NotDetected => ScanOutput::default(),
            DetectionStatus::Configured => scanner.scan(context),
        };
        (detection, output)
    }));
    let (detection, output) = outcome.unwrap_or_else(|payload| {
        let reason = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown error".into());
        let diagnostic = Diagnostic::new(
            DiagnosticLevel::Error,
            DiagnosticCode::Internal,
            Message::new("diagnostic.internalScannerError")
                .arg("scanner", &metadata.id)
                .arg("reason", reason),
        );
        (
            DetectionResult::not_detected(),
            ScanOutput {
                diagnostics: vec![diagnostic],
                ..ScanOutput::default()
            },
        )
    });
    ScanResult {
        scanner: metadata,
        detection,
        findings: analysis::analyze(output.findings, context),
        diagnostics: output.diagnostics,
        summary: output.summary,
    }
}

/// Scanners that run when no agent is selected explicitly.
pub fn default_scanners() -> Vec<&'static dyn AgentScanner> {
    scanners::REAL.to_vec()
}

/// All scanners, including the demo scanner.
pub fn all_scanners() -> Vec<&'static dyn AgentScanner> {
    scanners::REAL
        .iter()
        .chain(scanners::EXTRA)
        .copied()
        .collect()
}

/// Looks up a scanner by its id.
pub fn find_scanner(id: &str) -> Option<&'static dyn AgentScanner> {
    all_scanners()
        .into_iter()
        .find(|scanner| scanner.metadata().id == id)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::context::EnvSnapshot;
    use crate::platform::OperatingSystem;

    fn context() -> ScanContext {
        ScanContext {
            home_dir: PathBuf::from("/home/alice"),
            current_dir: None,
            operating_system: OperatingSystem::Linux,
            env: EnvSnapshot::default(),
            system_root: None,
        }
    }

    fn metadata(id: &str) -> ScannerMetadata {
        ScannerMetadata {
            id: id.into(),
            name: id.into(),
            description: String::new(),
            parser_version: "1".into(),
        }
    }

    struct Absent;

    impl AgentScanner for Absent {
        fn metadata(&self) -> ScannerMetadata {
            metadata("absent")
        }
        fn detect(&self, _: &ScanContext) -> DetectionResult {
            DetectionResult::not_detected()
        }
        fn scan(&self, _: &ScanContext) -> ScanOutput {
            panic!("scan must not run for undetected agents")
        }
    }

    struct Broken;

    impl AgentScanner for Broken {
        fn metadata(&self) -> ScannerMetadata {
            metadata("broken")
        }
        fn detect(&self, _: &ScanContext) -> DetectionResult {
            panic!("boom")
        }
        fn scan(&self, _: &ScanContext) -> ScanOutput {
            ScanOutput::default()
        }
    }

    #[test]
    fn undetected_agent_is_not_scanned() {
        let report = run_scan(&context(), &[&Absent]);
        let result = &report.results[0];
        assert_eq!(result.detection.status, DetectionStatus::NotDetected);
        assert!(result.findings.is_empty());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn panicking_scanner_becomes_internal_diagnostic() {
        let report = run_scan(&context(), &[&Broken, &Absent]);
        assert_eq!(report.results.len(), 2);
        let diagnostic = &report.results[0].diagnostics[0];
        assert_eq!(diagnostic.code, DiagnosticCode::Internal);
        assert_eq!(diagnostic.message.args["reason"], "boom");
        assert!(diagnostic.message.text.contains("boom"));
    }

    #[test]
    fn report_carries_format_and_tool_version() {
        let report = run_scan(&context(), &[]);
        assert_eq!(report.format_version, REPORT_FORMAT_VERSION);
        assert_eq!(report.tool.version, crate::VERSION);
        assert_eq!(report.operating_system, OperatingSystem::Linux);
    }

    #[test]
    fn demo_scanner_is_found_but_not_a_default() {
        assert!(find_scanner("demo").is_some());
        assert!(find_scanner("nope").is_none());
        assert!(
            default_scanners()
                .iter()
                .all(|scanner| scanner.metadata().id != "demo")
        );
    }

    #[test]
    fn scanner_ids_are_unique() {
        let mut ids: Vec<String> = all_scanners().iter().map(|s| s.metadata().id).collect();
        let count = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }
}
