//! Scanner for Claude Code (CLI, IDE extensions and desktop app).
//!
//! Specification: `docs/claude-code-scanner.md`.

mod mcp;
mod projects;
mod rules;
mod settings;

use serde_json::Value;

use crate::analysis::path::TargetPath;
use crate::analysis::resolve::resolve_path;
use crate::context::ScanContext;
use crate::model::{
    DetectionResult, DetectionStatus, Diagnostic, DiagnosticCode, DiagnosticLevel, Finding,
    FindingSource, Message, ScanOutput, ScannerMetadata, SourceDocumentation, SummaryEntry,
};
use crate::platform::OperatingSystem;
use crate::read::{self, JsonFile};
use crate::scanner::AgentScanner;

use projects::Project;
use settings::{Level, SettingsFile};

pub const ID: &str = "claude-code";
const PARSER_VERSION: &str = "1";

pub struct ClaudeCodeScanner;

impl AgentScanner for ClaudeCodeScanner {
    fn metadata(&self) -> ScannerMetadata {
        ScannerMetadata {
            id: ID.into(),
            name: "Claude Code".into(),
            description: "Settings, permission rules, sandbox and MCP servers of Claude Code"
                .into(),
            parser_version: PARSER_VERSION.into(),
        }
    }

    fn detect(&self, context: &ScanContext) -> DetectionResult {
        let Some(locations) = Locations::new(context) else {
            return DetectionResult::not_detected();
        };
        let evidence: Vec<_> = locations
            .detection_candidates()
            .into_iter()
            .filter(|path| read::exists(context, &path.to_string()))
            .map(|path| path.to_path_buf())
            .collect();
        if evidence.is_empty() {
            return DetectionResult::not_detected();
        }
        DetectionResult {
            status: DetectionStatus::Configured,
            evidence,
            version: projects::latest_session(context, &locations).and_then(|s| s.version),
        }
    }

    fn scan(&self, context: &ScanContext) -> ScanOutput {
        let Some(locations) = Locations::new(context) else {
            return ScanOutput::default();
        };
        let mut scan = Scan {
            context,
            meta: self.metadata(),
            locations,
            out: ScanOutput::default(),
        };
        scan.run();
        scan.out
    }
}

/// Where Claude Code keeps its files on the scanned system.
pub(crate) struct Locations {
    pub home: TargetPath,
    /// `~/.claude` or `$CLAUDE_CONFIG_DIR`.
    pub config_dir: TargetPath,
    /// Candidates for `.claude.json`, in lookup order.
    pub claude_json: Vec<TargetPath>,
    pub managed_dir: TargetPath,
}

impl Locations {
    fn new(context: &ScanContext) -> Option<Self> {
        let home = context.home()?;
        let os = context.operating_system;
        let custom_config_dir = context
            .env_var("CLAUDE_CONFIG_DIR")
            .and_then(|dir| resolve_path(dir, &home, None).ok());
        let default_config_dir = home.join(".claude").ok()?;
        let home_claude_json = home.join(".claude.json").ok()?;
        let claude_json = match &custom_config_dir {
            Some(dir) => vec![dir.join(".claude.json").ok()?, home_claude_json],
            None => vec![home_claude_json],
        };
        let managed_dir = match os {
            OperatingSystem::MacOs => "/Library/Application Support/ClaudeCode".to_string(),
            OperatingSystem::Linux => "/etc/claude-code".to_string(),
            OperatingSystem::Windows => format!(
                "{}\\ClaudeCode",
                context
                    .env_var("ProgramFiles")
                    .unwrap_or(r"C:\Program Files")
            ),
        };
        Some(Self {
            config_dir: custom_config_dir.unwrap_or(default_config_dir),
            claude_json,
            managed_dir: TargetPath::parse(os, &managed_dir).ok()?,
            home,
        })
    }

    fn file(dir: &TargetPath, name: &str) -> TargetPath {
        dir.join(name).expect("static file names are valid")
    }

    pub fn managed_settings(&self) -> TargetPath {
        Self::file(&self.managed_dir, "managed-settings.json")
    }

    pub fn managed_dropins(&self) -> TargetPath {
        Self::file(&self.managed_dir, "managed-settings.d")
    }

    pub fn managed_mcp(&self) -> TargetPath {
        Self::file(&self.managed_dir, "managed-mcp.json")
    }

    pub fn user_settings(&self) -> TargetPath {
        Self::file(&self.config_dir, "settings.json")
    }

    pub fn sessions_dir(&self) -> TargetPath {
        Self::file(&self.config_dir, "projects")
    }

    fn detection_candidates(&self) -> Vec<TargetPath> {
        let mut candidates = vec![self.config_dir.clone()];
        candidates.extend(self.claude_json.iter().cloned());
        candidates.push(self.managed_settings());
        candidates.push(self.managed_mcp());
        candidates
    }
}

/// State of one scan run.
pub(crate) struct Scan<'a> {
    pub context: &'a ScanContext,
    pub meta: ScannerMetadata,
    pub locations: Locations,
    pub out: ScanOutput,
}

/// The parts of `.claude.json` the scanner uses; everything else is dropped
/// right after parsing.
pub(crate) struct ClaudeJson {
    pub path: TargetPath,
    pub mcp_servers: Option<Value>,
    pub projects: serde_json::Map<String, Value>,
}

impl Scan<'_> {
    fn run(&mut self) {
        let managed = self.read_managed_settings();
        let user = self.read_settings(Level::User, self.locations.user_settings(), None);
        let claude_json = self.read_claude_json();
        let projects = projects::discover(self, claude_json.as_ref());

        let mut files: Vec<SettingsFile> = managed;
        files.extend(user);
        for (index, project) in projects.iter().enumerate() {
            files.extend(self.read_project_settings(index, project));
        }

        settings::evaluate(self, &files, &projects);
        mcp::evaluate(self, &files, &projects, claude_json.as_ref());
        self.out.summary.push(SummaryEntry::new(
            "projects",
            Message::new("summary.projects"),
            Message::raw(projects.len()),
        ));
    }

    pub fn os(&self) -> OperatingSystem {
        self.context.operating_system
    }

    pub fn diagnostic(&mut self, diagnostic: Diagnostic) {
        self.out.diagnostics.push(diagnostic);
    }

    pub fn finding(&mut self, finding: Finding) {
        self.out.findings.push(finding);
    }

    pub fn source(
        &self,
        config_kind: &str,
        documentation: SourceDocumentation,
        file: &TargetPath,
        pointer: &str,
    ) -> FindingSource {
        FindingSource::new(format!("{ID}/{config_kind}"), documentation, PARSER_VERSION)
            .file(file.to_path_buf())
            .pointer(pointer)
    }

    pub fn derived_source(&self) -> FindingSource {
        FindingSource::new(
            format!("{ID}/derived"),
            SourceDocumentation::Derived,
            PARSER_VERSION,
        )
    }

    /// Reads a JSON file; problems become diagnostics.
    pub fn read_json(&mut self, path: &TargetPath) -> Option<Value> {
        match read::read_json(self.context, &path.to_string()) {
            JsonFile::Missing => None,
            JsonFile::Parsed(value) => Some(value),
            JsonFile::Failed(diagnostic) => {
                self.diagnostic(diagnostic);
                None
            }
        }
    }

    fn read_settings(
        &mut self,
        level: Level,
        path: TargetPath,
        project: Option<usize>,
    ) -> Option<SettingsFile> {
        let value = self.read_json(&path)?;
        if !value.is_object() {
            self.diagnostic(unexpected_type(&path, "", "an object"));
            return None;
        }
        Some(SettingsFile {
            level,
            path,
            value,
            project,
        })
    }

    /// `managed-settings.json` plus drop-ins, highest precedence first.
    fn read_managed_settings(&mut self) -> Vec<SettingsFile> {
        let os = self.os();
        if os == OperatingSystem::Windows {
            let legacy = r"C:\ProgramData\ClaudeCode\managed-settings.json";
            if read::exists(self.context, legacy) {
                self.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Warning,
                        DiagnosticCode::DeprecatedLocation,
                        Message::new("diagnostic.deprecatedLocation").arg("path", legacy),
                    )
                    .file(legacy),
                );
            }
        }
        if os == OperatingSystem::MacOs {
            let profile = "/Library/Managed Preferences/com.anthropic.claudecode.plist";
            if read::exists(self.context, profile) {
                self.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Info,
                        DiagnosticCode::UnevaluatedSource,
                        Message::new("diagnostic.mdmProfileDetected").arg("path", profile),
                    )
                    .file(profile),
                );
            }
        }

        let mut files = Vec::new();
        let dropins = self.locations.managed_dropins();
        let names: Vec<String> = read::list_dir(self.context, &dropins.to_string())
            .into_iter()
            .filter(|name| !name.starts_with('.') && name.ends_with(".json"))
            .collect();
        // Later drop-ins override earlier ones and the base file.
        for name in names.iter().rev() {
            let path = Locations::file(&dropins, name);
            files.extend(self.read_settings(Level::Managed, path, None));
        }
        files.extend(self.read_settings(Level::Managed, self.locations.managed_settings(), None));
        if files.iter().any(|f| f.value.get("policyHelper").is_some()) {
            self.diagnostic(Diagnostic::new(
                DiagnosticLevel::Info,
                DiagnosticCode::UnevaluatedSource,
                Message::new("diagnostic.policyHelper"),
            ));
        }
        files
    }

    fn read_claude_json(&mut self) -> Option<ClaudeJson> {
        let path = self
            .locations
            .claude_json
            .iter()
            .find(|path| read::exists(self.context, &path.to_string()))?
            .clone();
        let mut value = self.read_json(&path)?;
        let object = value.as_object_mut()?;
        let projects = match object.remove("projects") {
            Some(Value::Object(projects)) => projects,
            _ => serde_json::Map::new(),
        };
        Some(ClaudeJson {
            mcp_servers: object.remove("mcpServers"),
            projects,
            path,
        })
    }

    fn read_project_settings(&mut self, index: usize, project: &Project) -> Vec<SettingsFile> {
        let mut files = Vec::new();
        let claude_dir = Locations::file(&project.path, ".claude");
        files.extend(self.read_settings(
            Level::ProjectLocal,
            Locations::file(&claude_dir, "settings.local.json"),
            Some(index),
        ));
        if let Some(root) = projects::git_root(self.context, &project.path)
            && !root.same_path(&project.path)
        {
            let local = Locations::file(&Locations::file(&root, ".claude"), "settings.local.json");
            files.extend(self.read_settings(Level::ProjectLocal, local, Some(index)));
        }
        files.extend(self.read_settings(
            Level::Project,
            Locations::file(&claude_dir, "settings.json"),
            Some(index),
        ));
        files
    }
}

/// Escapes a key for use in a JSON pointer (RFC 6901).
pub(crate) fn pointer_key(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

pub(crate) fn unexpected_type(file: &TargetPath, pointer: &str, expected: &str) -> Diagnostic {
    let shown = if pointer.is_empty() { "/" } else { pointer };
    Diagnostic::new(
        DiagnosticLevel::Warning,
        DiagnosticCode::UnexpectedType,
        Message::new("diagnostic.unexpectedType")
            .arg("pointer", shown)
            .arg("expected", expected),
    )
    .file(file.to_path_buf())
    .pointer(shown)
}
