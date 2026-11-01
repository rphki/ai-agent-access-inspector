//! Results of a scan, per scanner and as a whole report.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{Diagnostic, Finding, Message};
use crate::platform::OperatingSystem;

/// Version of the JSON report format. May change incompatibly before 1.0.
pub const REPORT_FORMAT_VERSION: &str = "0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerMetadata {
    /// Stable machine readable id, e.g. `claude-code`.
    pub id: String,
    /// Name shown to users, e.g. `Claude Code`.
    pub name: String,
    pub description: String,
    /// Version of the scanner's parsing logic, recorded in every source.
    pub parser_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectionResult {
    pub status: DetectionStatus,
    /// Files or directories that led to the detection.
    pub evidence: Vec<PathBuf>,
    /// Agent version, if it could be determined.
    pub version: Option<String>,
}

impl DetectionResult {
    pub fn not_detected() -> Self {
        Self {
            status: DetectionStatus::NotDetected,
            evidence: Vec::new(),
            version: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectionStatus {
    NotDetected,
    /// Configuration exists; the program itself was not looked for.
    Configured,
}

/// What a scanner produces. The runner adds metadata and detection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOutput {
    pub findings: Vec<Finding>,
    pub diagnostics: Vec<Diagnostic>,
    pub summary: Vec<SummaryEntry>,
}

/// A key fact about an agent's configuration, e.g. its permission mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryEntry {
    /// Machine readable identifier of the fact, e.g. `permissionMode`.
    pub key: String,
    pub label: Message,
    pub value: Message,
}

impl SummaryEntry {
    pub fn new(key: impl Into<String>, label: Message, value: Message) -> Self {
        Self {
            key: key.into(),
            label,
            value,
        }
    }
}

/// Result of one scanner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub scanner: ScannerMetadata,
    pub detection: DetectionResult,
    pub findings: Vec<Finding>,
    pub diagnostics: Vec<Diagnostic>,
    pub summary: Vec<SummaryEntry>,
}

/// Result of a whole scan run, as exported and sent to the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub format_version: String,
    pub tool: ToolInfo,
    pub operating_system: OperatingSystem,
    pub results: Vec<ScanResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub name: String,
    pub version: String,
}

impl ToolInfo {
    pub fn current() -> Self {
        Self {
            name: crate::APP_NAME.into(),
            version: crate::VERSION.into(),
        }
    }
}
