//! Diagnostics describe problems while scanning. They never abort a scan.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub code: DiagnosticCode,
    pub message: String,
    pub file: Option<PathBuf>,
    /// JSON pointer to the affected value inside `file`.
    pub pointer: Option<String>,
}

impl Diagnostic {
    pub fn new(level: DiagnosticLevel, code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            level,
            code,
            message: message.into(),
            file: None,
            pointer: None,
        }
    }

    pub fn file(mut self, file: impl Into<PathBuf>) -> Self {
        self.file = Some(file.into());
        self
    }

    pub fn pointer(mut self, pointer: impl Into<String>) -> Self {
        self.pointer = Some(pointer.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticCode {
    UnreadableFile,
    InvalidJson,
    UnexpectedType,
    UnknownKey,
    UnparsableRule,
    UnresolvedVariable,
    /// A file at a location the agent no longer reads.
    DeprecatedLocation,
    /// A source exists but is not evaluated, e.g. an MDM profile.
    UnevaluatedSource,
    /// A scanner failed unexpectedly.
    Internal,
    Other,
}
