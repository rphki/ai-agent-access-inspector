//! A single finding: one path or rule that grants, limits or hints at
//! filesystem access for an agent.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::message::Message;
use super::scan::ScannerMetadata;

/// One path or rule found in an agent's configuration or environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    /// Stable identifier, derived from agent, source and target.
    pub id: String,
    pub agent_id: String,
    pub agent_name: String,
    /// Who holds the access: the agent itself or e.g. one of its MCP servers.
    pub subject: FindingSubject,
    pub target: FindingTarget,
    pub effect: RuleEffect,
    pub access: AccessKind,
    pub status: FindingStatus,
    pub scope: FindingScope,
    pub source: FindingSource,
    /// Further sources with the same content, merged by deduplication.
    pub additional_sources: Vec<FindingSource>,
    /// Explanation of what this finding means.
    pub explanation: Message,
    pub confidence: Confidence,
    pub path_state: PathState,
    pub warnings: Vec<Warning>,
    /// Relations to other findings of the same agent.
    pub related: Vec<RelatedFinding>,
}

impl Finding {
    /// Starts a finding with neutral defaults; see [`FindingBuilder`].
    pub fn builder(
        agent: &ScannerMetadata,
        scope: FindingScope,
        target: FindingTarget,
        source: FindingSource,
    ) -> FindingBuilder {
        FindingBuilder {
            finding: Finding {
                id: String::new(),
                agent_id: agent.id.clone(),
                agent_name: agent.name.clone(),
                subject: FindingSubject::Agent,
                target,
                effect: RuleEffect::NotApplicable,
                access: AccessKind::Unknown,
                status: FindingStatus::Unknown,
                scope,
                source,
                additional_sources: Vec::new(),
                explanation: Message::new("finding.noExplanation"),
                confidence: Confidence::Low,
                path_state: PathState::default(),
                warnings: Vec::new(),
                related: Vec::new(),
            },
        }
    }
}

/// Builder for [`Finding`]. The id is computed in [`FindingBuilder::build`].
#[derive(Debug, Clone)]
pub struct FindingBuilder {
    finding: Finding,
}

impl FindingBuilder {
    pub fn subject(mut self, subject: FindingSubject) -> Self {
        self.finding.subject = subject;
        self
    }

    pub fn effect(mut self, effect: RuleEffect) -> Self {
        self.finding.effect = effect;
        self
    }

    pub fn access(mut self, access: AccessKind) -> Self {
        self.finding.access = access;
        self
    }

    pub fn status(mut self, status: FindingStatus) -> Self {
        self.finding.status = status;
        self
    }

    pub fn confidence(mut self, confidence: Confidence) -> Self {
        self.finding.confidence = confidence;
        self
    }

    pub fn explanation(mut self, explanation: Message) -> Self {
        self.finding.explanation = explanation;
        self
    }

    pub fn path_state(mut self, path_state: PathState) -> Self {
        self.finding.path_state = path_state;
        self
    }

    pub fn warning(mut self, warning: Warning) -> Self {
        self.finding.warnings.push(warning);
        self
    }

    pub fn build(mut self) -> Finding {
        self.finding.id = finding_id(&self.finding);
        self.finding
    }
}

/// Derives a stable id so that repeated scans of unchanged configuration
/// produce identical ids.
fn finding_id(finding: &Finding) -> String {
    let subject = match &finding.subject {
        FindingSubject::Agent => "",
        FindingSubject::McpServer { name } => name,
    };
    let file = finding
        .source
        .file
        .as_ref()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();
    let parts = [
        finding.agent_id.as_str(),
        subject,
        file.as_str(),
        finding.source.pointer.as_deref().unwrap_or(""),
        finding.target.raw.as_str(),
    ];
    format!("{}-{:016x}", finding.agent_id, fnv1a64(&parts))
}

/// FNV-1a over the given parts, separated by a zero byte. Not
/// cryptographic; it only needs to be stable across runs and platforms.
fn fnv1a64(parts: &[&str]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for part in parts {
        for byte in part.bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(PRIME);
        }
    }
    hash
}

/// Who holds the access described by a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FindingSubject {
    Agent,
    McpServer { name: String },
}

/// The path or rule a finding refers to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingTarget {
    /// Value exactly as written in the source, e.g. `Read(~/projekt/**)`.
    pub raw: String,
    pub pattern_kind: PatternKind,
    /// Path the rule is rooted at, once it could be determined.
    pub base_path: Option<PathBuf>,
    /// Path as shown to the user.
    pub display_path: String,
}

impl FindingTarget {
    /// A target that refers to a concrete path.
    pub fn path(raw: impl Into<String>, kind: PatternKind, base_path: PathBuf) -> Self {
        let display_path = base_path.to_string_lossy().into_owned();
        Self {
            raw: raw.into(),
            pattern_kind: kind,
            base_path: Some(base_path),
            display_path,
        }
    }

    /// A target without a resolvable path, e.g. a whole tool or an
    /// unparsable rule.
    pub fn unresolved(raw: impl Into<String>, kind: PatternKind) -> Self {
        let raw = raw.into();
        Self {
            display_path: raw.clone(),
            raw,
            pattern_kind: kind,
            base_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PatternKind {
    ExactPath,
    Directory,
    Glob,
    /// Covers everything the subject can reach, e.g. an unsandboxed shell.
    Everything,
    Unparsed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RuleEffect {
    Allow,
    Ask,
    Deny,
    /// The finding does not stem from a rule, e.g. an observed project path.
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AccessKind {
    Read,
    Write,
    Execute,
    ReadWrite,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingStatus {
    /// Explicitly written in a configuration.
    Configured,
    /// Seen in stored project or session information.
    Observed,
    /// Reachable because of process context or operating system rights.
    Potential,
    /// Configured but currently not reachable.
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FindingScope {
    Managed,
    User,
    Project,
    ProjectLocal,
    Session,
    Runtime,
    OperatingSystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

/// Where a finding comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FindingSource {
    /// File the value was read from; `None` for derived findings.
    pub file: Option<PathBuf>,
    /// Kind of configuration, e.g. `claude-code/settings`.
    pub config_kind: String,
    pub documentation: SourceDocumentation,
    /// JSON pointer to the value inside `file`.
    pub pointer: Option<String>,
    /// Value as written in the source, before normalisation.
    pub raw_value: Option<String>,
    /// What relative patterns in this value are resolved against.
    pub anchor: Option<PathAnchor>,
    pub parser_version: String,
    /// Short excerpt for evidence, never containing secrets.
    pub excerpt: Option<String>,
}

impl FindingSource {
    pub fn new(
        config_kind: impl Into<String>,
        documentation: SourceDocumentation,
        parser_version: impl Into<String>,
    ) -> Self {
        Self {
            file: None,
            config_kind: config_kind.into(),
            documentation,
            pointer: None,
            raw_value: None,
            anchor: None,
            parser_version: parser_version.into(),
            excerpt: None,
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

    pub fn raw_value(mut self, raw_value: impl Into<String>) -> Self {
        self.raw_value = Some(raw_value.into());
        self
    }

    pub fn anchor(mut self, anchor: PathAnchor) -> Self {
        self.anchor = Some(anchor);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceDocumentation {
    /// Described in the agent's official documentation.
    Documented,
    /// Observed in installations but not (fully) documented.
    Heuristic,
    /// Computed by the inspector rather than read from a file.
    Derived,
}

/// Base that relative patterns are resolved against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PathAnchor {
    FilesystemRoot,
    Home,
    WorkingDirectory,
    Directory { path: PathBuf },
}

/// State of the path on disk. Filled in by the path analysis.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathState {
    pub existence: Existence,
    pub kind: PathKind,
    pub symlink_target: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Existence {
    Exists,
    Missing,
    /// The check failed, e.g. because of missing permissions.
    Unknown,
    #[default]
    NotChecked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PathKind {
    File,
    Directory,
    Other,
    #[default]
    Unknown,
}

/// A security relevant remark attached to a finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    pub kind: WarningKind,
    pub severity: Severity,
    pub message: Message,
}

impl Warning {
    pub fn new(kind: WarningKind, severity: Severity, message: Message) -> Self {
        Self {
            kind,
            severity,
            message,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WarningKind {
    HomeDirectory,
    FilesystemRoot,
    WholeDrive,
    BroadPath,
    MissingPath,
    SymlinkElsewhere,
    ConflictingRules,
    DangerousMode,
    UnsandboxedShell,
    /// A location that typically holds credentials, e.g. `~/.ssh`.
    SensitiveLocation,
    /// A probable mistake in the configuration, e.g. a misanchored path.
    LikelyMisconfigured,
    /// The configuration exists but does not take effect.
    NotEffective {
        reason: IneffectiveReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IneffectiveReason {
    /// The project's trust dialog has not been accepted yet.
    TrustNotAccepted,
    /// Only managed rules are honoured.
    ManagedRulesOnly,
    /// This scope may not set the value.
    ScopeNotAllowed,
    /// The rule can never match.
    RuleNeverMatches,
    /// A higher scope sets a different value.
    OverriddenByHigherScope,
    /// Not supported on this operating system.
    PlatformUnsupported,
}

/// A relation from one finding to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelatedFinding {
    pub relation: Relation,
    pub finding_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Relation {
    /// The other finding's path lies below this one.
    Contains,
    /// This finding's path lies below the other one.
    ContainedIn,
    /// A deny rule that takes precedence over this finding.
    OverriddenBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> ScannerMetadata {
        ScannerMetadata {
            id: "test".into(),
            name: "Test".into(),
            description: String::new(),
            parser_version: "1".into(),
        }
    }

    fn finding(raw: &str) -> Finding {
        let source = FindingSource::new("test/settings", SourceDocumentation::Documented, "1")
            .file("/etc/test.json")
            .pointer("/permissions/allow/0");
        Finding::builder(
            &agent(),
            FindingScope::User,
            FindingTarget::path(raw, PatternKind::Directory, PathBuf::from("/srv")),
            source,
        )
        .build()
    }

    #[test]
    fn id_is_stable_and_depends_on_target() {
        assert_eq!(finding("/srv").id, finding("/srv").id);
        assert_ne!(finding("/srv").id, finding("/srv/other").id);
        assert!(finding("/srv").id.starts_with("test-"));
    }

    #[test]
    fn fnv1a64_matches_reference_value() {
        // FNV-1a 64 of "a" is 0xaf63dc4c8601ec8c. The zero separator that
        // follows only multiplies by the prime, since x ^ 0 == x.
        let expected = 0xaf63_dc4c_8601_ec8c_u64.wrapping_mul(0x0000_0100_0000_01b3);
        assert_eq!(fnv1a64(&["a"]), expected);
    }

    #[test]
    fn serializes_with_camel_case_names() {
        let json = serde_json::to_value(finding("/srv")).unwrap();
        assert_eq!(json["agentId"], "test");
        assert_eq!(json["subject"]["kind"], "agent");
        assert_eq!(json["target"]["patternKind"], "directory");
        assert_eq!(json["scope"], "user");
        assert_eq!(json["pathState"]["existence"], "notChecked");
        assert_eq!(json["source"]["pointer"], "/permissions/allow/0");
    }

    #[test]
    fn round_trips_through_json() {
        let original = finding("/srv");
        let json = serde_json::to_string(&original).unwrap();
        let parsed: Finding = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, original);
    }

    #[test]
    fn not_effective_warning_carries_reason() {
        let warning = Warning::new(
            WarningKind::NotEffective {
                reason: IneffectiveReason::TrustNotAccepted,
            },
            Severity::Info,
            Message::new("value.raw"),
        );
        let json = serde_json::to_value(warning).unwrap();
        assert_eq!(json["kind"]["type"], "notEffective");
        assert_eq!(json["kind"]["reason"], "trustNotAccepted");
    }
}
