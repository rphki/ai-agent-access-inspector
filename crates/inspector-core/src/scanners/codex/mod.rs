//! Scanner for the ChatGPT desktop app and the Codex CLI, which share
//! `~/.codex`.
//!
//! Specification: `docs/codex-scanner.md`.

mod mcp;
mod projects;
mod sandbox;

use serde_json::Value;

use crate::analysis::path::TargetPath;
use crate::analysis::resolve::resolve_path;
use crate::context::ScanContext;
use crate::model::{
    DetectionResult, DetectionStatus, Diagnostic, DiagnosticCode, DiagnosticLevel, Finding,
    FindingScope, FindingSource, Message, ScanOutput, ScannerMetadata, SourceDocumentation,
    SummaryEntry,
};
use crate::platform::OperatingSystem;
use crate::read::{self, ParsedFile};
use crate::scanner::AgentScanner;

use projects::Project;

pub const ID: &str = "codex";
const PARSER_VERSION: &str = "1";

/// Package manifest of the Codex CLI bundled with the ChatGPT app (macOS).
const APP_PACKAGE: &str =
    "/Applications/ChatGPT.app/Contents/Resources/codex-cli/codex-package.json";
const MDM_PROFILE: &str = "/Library/Managed Preferences/com.openai.codex.plist";

pub struct CodexScanner;

impl AgentScanner for CodexScanner {
    fn metadata(&self) -> ScannerMetadata {
        ScannerMetadata {
            id: ID.into(),
            name: "ChatGPT / Codex".into(),
            description: "Sandbox, permission profiles, projects, MCP servers and plugins of the ChatGPT app and the Codex CLI".into(),
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
            version: app_version(context, &locations),
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

/// Where Codex keeps its files on the scanned system.
pub(crate) struct Locations {
    pub home: TargetPath,
    /// `~/.codex` or `$CODEX_HOME`.
    pub codex_home: TargetPath,
    /// `/etc/codex/config.toml`; Unix only.
    pub system_config: Option<TargetPath>,
    pub managed_config: TargetPath,
    pub requirements: TargetPath,
    /// The app's package manifest (macOS only).
    pub app_package: Option<TargetPath>,
}

impl Locations {
    fn new(context: &ScanContext) -> Option<Self> {
        let home = context.home()?;
        let os = context.operating_system;
        let codex_home = context
            .env_var("CODEX_HOME")
            .and_then(|dir| resolve_path(dir, &home, None).ok())
            .unwrap_or(home.join(".codex").ok()?);
        let parse = |path: &str| TargetPath::parse(os, path).ok();
        let (system_config, managed_config, requirements) = match os {
            OperatingSystem::Windows => {
                let program_data = context.env_var("ProgramData").unwrap_or(r"C:\ProgramData");
                (
                    None,
                    Self::file(&codex_home, "managed_config.toml"),
                    parse(&format!(r"{program_data}\OpenAI\Codex\requirements.toml"))?,
                )
            }
            OperatingSystem::MacOs | OperatingSystem::Linux => (
                parse("/etc/codex/config.toml"),
                parse("/etc/codex/managed_config.toml")?,
                parse("/etc/codex/requirements.toml")?,
            ),
        };
        Some(Self {
            app_package: (os == OperatingSystem::MacOs)
                .then(|| parse(APP_PACKAGE))
                .flatten(),
            home,
            codex_home,
            system_config,
            managed_config,
            requirements,
        })
    }

    fn file(dir: &TargetPath, name: &str) -> TargetPath {
        dir.join(name).expect("static file names are valid")
    }

    pub fn user_config(&self) -> TargetPath {
        Self::file(&self.codex_home, "config.toml")
    }

    pub fn app_state(&self) -> TargetPath {
        Self::file(&self.codex_home, ".codex-global-state.json")
    }

    fn detection_candidates(&self) -> Vec<TargetPath> {
        let mut candidates = vec![self.codex_home.clone()];
        candidates.extend(self.system_config.clone());
        candidates.push(self.managed_config.clone());
        candidates.push(self.requirements.clone());
        candidates.extend(self.app_package.clone());
        candidates
    }
}

/// The version from the app's bundled CLI manifest, if present.
fn app_version(context: &ScanContext, locations: &Locations) -> Option<String> {
    let package = locations.app_package.as_ref()?;
    match read::read_json(context, &package.to_string()) {
        ParsedFile::Parsed(value) => value.get("version")?.as_str().map(String::from),
        _ => None,
    }
}

/// Configuration levels, see the specification, section 3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    /// `managed_config.toml`; overrides everything else.
    Managed,
    Project,
    User,
    /// `/etc/codex/config.toml`; below the user configuration.
    System,
}

impl Level {
    pub fn scope(self) -> FindingScope {
        match self {
            Level::Managed | Level::System => FindingScope::Managed,
            Level::Project => FindingScope::Project,
            Level::User => FindingScope::User,
        }
    }

    pub fn config_kind(self) -> &'static str {
        match self {
            Level::Managed => "managed-config",
            Level::Project => "project-config",
            Level::User => "config",
            Level::System => "system-config",
        }
    }
}

/// One parsed `config.toml`-style file.
#[derive(Debug)]
pub(crate) struct Layer {
    pub level: Level,
    pub path: TargetPath,
    pub value: Value,
    /// Index of the project for project configuration.
    pub project: Option<usize>,
}

/// The layers that apply in one place, highest precedence first.
#[derive(Debug, Clone)]
pub(crate) struct View<'a> {
    layers: Vec<&'a Layer>,
}

impl<'a> View<'a> {
    /// Everything outside a specific project.
    pub fn global(layers: &'a [Layer]) -> Self {
        Self {
            layers: layers.iter().filter(|l| l.project.is_none()).collect(),
        }
    }

    /// Everything that applies in one project: managed, the project's own
    /// files (closest first), user, system.
    pub fn project(layers: &'a [Layer], project: usize) -> Self {
        Self {
            layers: layers
                .iter()
                .filter(|l| l.project.is_none_or(|p| p == project))
                .collect(),
        }
    }

    pub fn layers(&self) -> &[&'a Layer] {
        &self.layers
    }

    /// The value from the layer with the highest precedence.
    pub fn get(&self, pointer: &str) -> Option<(&'a Layer, &'a Value)> {
        self.layers
            .iter()
            .find_map(|layer| layer.value.pointer(pointer).map(|value| (*layer, value)))
    }

    pub fn get_str(&self, pointer: &str) -> Option<(&'a Layer, &'a str)> {
        let (layer, value) = self.get(pointer)?;
        Some((layer, value.as_str()?))
    }

    pub fn get_bool(&self, pointer: &str) -> Option<bool> {
        self.get(pointer)?.1.as_bool()
    }
}

/// `requirements.toml`: constraints that local configuration cannot relax.
pub(crate) struct Requirements {
    pub path: TargetPath,
    pub value: Value,
}

/// State of one scan run.
pub(crate) struct Scan<'a> {
    pub context: &'a ScanContext,
    pub meta: ScannerMetadata,
    pub locations: Locations,
    pub out: ScanOutput,
}

impl Scan<'_> {
    fn run(&mut self) {
        if self.os() == OperatingSystem::MacOs && read::exists(self.context, MDM_PROFILE) {
            self.diagnostic(
                Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticCode::UnevaluatedSource,
                    Message::new("diagnostic.mdmProfileDetected").arg("path", MDM_PROFILE),
                )
                .file(MDM_PROFILE),
            );
        }
        let requirements = self.read_requirements();
        let mut layers = Vec::new();
        let managed = self.locations.managed_config.clone();
        layers.extend(self.read_layer(Level::Managed, managed, None));
        layers.extend(self.read_layer(Level::User, self.locations.user_config(), None));
        if let Some(system) = self.locations.system_config.clone() {
            layers.extend(self.read_layer(Level::System, system, None));
        }

        let (projects, app_mode) = projects::discover(self, &View::global(&layers));
        for (index, project) in projects.iter().enumerate() {
            let files = self.read_project_layers(index, project);
            // Project files rank below managed and above user configuration.
            let at = layers
                .iter()
                .position(|l| l.level != Level::Managed && l.level != Level::Project)
                .unwrap_or(layers.len());
            layers.splice(at..at, files);
        }

        let policies = sandbox::evaluate(
            self,
            &layers,
            &projects,
            requirements.as_ref(),
            app_mode.as_ref(),
        );
        projects::findings(self, &projects, &policies);
        mcp::evaluate(self, &layers, requirements.as_ref());
        mcp::plugins(self, &View::global(&layers), requirements.as_ref());
        self.summary(&layers, requirements.as_ref(), projects.len());
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

    /// An absolute or `~` path from the configuration or the app state.
    pub fn target_path(&self, raw: &str) -> Option<TargetPath> {
        resolve_path(raw, &self.locations.home, None).ok()
    }

    fn read_toml(&mut self, path: &TargetPath) -> Option<Value> {
        let value = match read::read_toml(self.context, &path.to_string()) {
            ParsedFile::Missing => return None,
            ParsedFile::Parsed(value) => value,
            ParsedFile::Failed(diagnostic) => {
                self.diagnostic(diagnostic);
                return None;
            }
        };
        Some(value)
    }

    pub fn read_json(&mut self, path: &TargetPath) -> Option<Value> {
        match read::read_json(self.context, &path.to_string()) {
            ParsedFile::Missing => None,
            ParsedFile::Parsed(value) => Some(value),
            ParsedFile::Failed(diagnostic) => {
                self.diagnostic(diagnostic);
                None
            }
        }
    }

    fn read_layer(
        &mut self,
        level: Level,
        path: TargetPath,
        project: Option<usize>,
    ) -> Option<Layer> {
        let value = self.read_toml(&path)?;
        Some(Layer {
            level,
            path,
            value,
            project,
        })
    }

    fn read_requirements(&mut self) -> Option<Requirements> {
        let path = self.locations.requirements.clone();
        let value = self.read_toml(&path)?;
        Some(Requirements { path, value })
    }

    /// `.codex/config.toml` from the project directory up to its root,
    /// closest first. Untrusted projects are skipped with a diagnostic.
    fn read_project_layers(&mut self, index: usize, project: &Project) -> Vec<Layer> {
        let root = crate::scanners::claude_code::projects::git_root(self.context, &project.path)
            .unwrap_or_else(|| project.path.clone());
        let user_config = self.locations.user_config();
        let mut files = Vec::new();
        let mut dir = Some(project.path.clone());
        while let Some(current) = dir {
            let file = Locations::file(&Locations::file(&current, ".codex"), "config.toml");
            // In the home directory this is the user configuration.
            if !file.same_path(&user_config) && read::exists(self.context, &file.to_string()) {
                if project.trusted {
                    files.extend(self.read_layer(Level::Project, file, Some(index)));
                } else {
                    self.diagnostic(
                        Diagnostic::new(
                            DiagnosticLevel::Info,
                            DiagnosticCode::IgnoredSetting,
                            Message::new("diagnostic.codex.untrustedProjectConfig")
                                .arg("path", &file),
                        )
                        .file(file.to_path_buf()),
                    );
                }
            }
            if current.same_path(&root) {
                break;
            }
            dir = current.parent();
        }
        files
    }

    fn summary(&mut self, layers: &[Layer], requirements: Option<&Requirements>, projects: usize) {
        let global = View::global(layers);
        let approval = match global.get("/approval_policy") {
            Some((_, Value::String(policy))) => Message::raw(policy),
            Some((_, Value::Object(_))) => Message::raw("granular"),
            _ => Message::raw("on-request"),
        };
        let network = if global.get_bool("/sandbox_workspace_write/network_access") == Some(true) {
            Message::new("value.enabled")
        } else {
            Message::new("value.disabled")
        };
        let profile_files = read::list_dir(self.context, &self.locations.codex_home.to_string())
            .into_iter()
            .filter(|name| name.ends_with(".config.toml"))
            .count();
        let managed = layers
            .iter()
            .filter(|l| matches!(l.level, Level::Managed | Level::System))
            .count()
            + usize::from(requirements.is_some());
        let installation = if self
            .locations
            .app_package
            .as_ref()
            .is_some_and(|p| read::exists(self.context, &p.to_string()))
        {
            Message::new("value.codex.app")
        } else {
            Message::new("value.codex.cli")
        };
        let count = |n: usize| {
            if n == 0 {
                Message::new("value.none")
            } else {
                Message::raw(n)
            }
        };
        let mut entries = vec![
            ("installation", installation),
            ("approvalPolicy", approval),
            ("networkAccess", network),
        ];
        if self.os() == OperatingSystem::Windows {
            let windows = global
                .get_str("/windows/sandbox")
                .map_or_else(|| Message::new("value.default"), |(_, v)| Message::raw(v));
            entries.push(("windowsSandbox", windows));
        }
        entries.push(("profileFiles", count(profile_files)));
        entries.push(("managedSources", count(managed)));
        for (key, value) in entries {
            self.out.summary.push(SummaryEntry::new(
                key,
                Message::new(&format!("summary.codex.{key}")),
                value,
            ));
        }
        self.out.summary.push(SummaryEntry::new(
            "projects",
            Message::new("summary.projects"),
            Message::raw(projects),
        ));
    }
}

/// Escapes a key for use in a JSON pointer (RFC 6901).
pub(crate) fn pointer_key(key: &str) -> String {
    crate::scanners::claude_code::pointer_key(key)
}

pub(crate) fn unexpected_type(file: &TargetPath, pointer: &str, expected: &str) -> Diagnostic {
    crate::scanners::claude_code::unexpected_type(file, pointer, expected)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::context::EnvSnapshot;
    use crate::model::{AccessKind, Finding, RuleEffect, ScanResult};
    use crate::test_support::TempDir;

    struct System {
        dir: TempDir,
        context: ScanContext,
    }

    impl System {
        fn new(name: &str) -> Self {
            let dir = TempDir::new(name);
            let mut env = EnvSnapshot::default();
            env.set("TMPDIR", "/tmp/alice");
            let context = ScanContext {
                home_dir: PathBuf::from("/home/alice"),
                current_dir: None,
                operating_system: OperatingSystem::Linux,
                env,
                system_root: Some(dir.path().join("system")),
            };
            Self { dir, context }
        }

        fn write(&self, target: &str, content: &str) -> &Self {
            let host = self.context.host_path(target);
            std::fs::create_dir_all(host.parent().unwrap()).unwrap();
            std::fs::write(host, content).unwrap();
            self
        }

        fn mkdir(&self, target: &str) -> &Self {
            std::fs::create_dir_all(self.context.host_path(target)).unwrap();
            self
        }

        fn scan(&self) -> ScanResult {
            let _ = &self.dir;
            crate::scanner::run_scan(&self.context, &[&CodexScanner])
                .results
                .remove(0)
        }
    }

    fn find<'a>(result: &'a ScanResult, explanation: &str) -> Vec<&'a Finding> {
        result
            .findings
            .iter()
            .filter(|f| f.explanation.key == explanation)
            .collect()
    }

    #[test]
    fn listed_project_defaults_to_workspace_write_with_temp_and_protected_paths() {
        let system = System::new("codex-default");
        system
            .write(
                "/home/alice/.codex/config.toml",
                "[projects.\"/home/alice/app\"]\ntrust_level = \"trusted\"\n",
            )
            .mkdir("/home/alice/app/.git")
            .mkdir("/home/alice/app/.aws");
        let result = system.scan();
        let project = find(&result, "finding.codex.workingDirectory.write");
        assert_eq!(project.len(), 1);
        assert_eq!(project[0].access, AccessKind::ReadWrite);
        assert_eq!(find(&result, "finding.codex.protectedPath").len(), 2);
        assert_eq!(find(&result, "finding.codex.tmpdir").len(), 1);
        assert_eq!(find(&result, "finding.codex.slashTmp").len(), 1);
        assert_eq!(find(&result, "finding.codex.readEverything").len(), 1);
    }

    #[test]
    fn requirements_reject_full_access_and_untrusted_project_config_is_ignored() {
        let system = System::new("codex-requirements");
        system
            .write(
                "/home/alice/.codex/config.toml",
                "sandbox_mode = \"danger-full-access\"\n[projects.\"/srv/repo\"]\ntrust_level = \"untrusted\"\n",
            )
            .write("/srv/repo/.codex/config.toml", "sandbox_mode = \"danger-full-access\"\n")
            .write(
                "/etc/codex/requirements.toml",
                "allowed_sandbox_modes = [\"read-only\", \"workspace-write\"]\n",
            );
        let result = system.scan();
        assert!(find(&result, "finding.codex.fullAccess").is_empty());
        let project = find(&result, "finding.codex.workingDirectory.read");
        assert_eq!(project[0].explanation.args["mode"], "read-only");
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.key == "diagnostic.codex.untrustedProjectConfig")
        );
    }

    #[test]
    fn writable_roots_resolve_relative_to_the_file_and_are_replaced_by_higher_layers() {
        let system = System::new("codex-roots");
        system
            .write(
                "/home/alice/.codex/config.toml",
                "sandbox_mode = \"workspace-write\"\n[sandbox_workspace_write]\nwritable_roots = [\"data\"]\n",
            )
            .write(
                "/etc/codex/managed_config.toml",
                "[sandbox_workspace_write]\nwritable_roots = [\"/srv/shared\"]\n",
            );
        let result = system.scan();
        let roots = find(&result, "finding.codex.writableRoot");
        let user = roots
            .iter()
            .find(|f| f.target.raw == "data")
            .expect("user root");
        assert_eq!(
            user.target.base_path.as_deref(),
            Some(std::path::Path::new("/home/alice/.codex/data"))
        );
        assert!(
            user.warnings
                .iter()
                .any(|w| w.message.key == "warning.codex.writableRootsReplaced")
        );
        let managed = roots
            .iter()
            .find(|f| f.target.raw == "/srv/shared")
            .unwrap();
        assert!(
            managed
                .warnings
                .iter()
                .all(|w| w.message.key == "warning.missingPath")
        );
    }

    #[test]
    fn permission_profile_entries_become_findings() {
        let system = System::new("codex-profile");
        system.write(
            "/home/alice/.codex/config.toml",
            "default_permissions = \"team\"\n\
             [permissions.team]\nextends = \":workspace\"\n\
             [permissions.team.filesystem]\n\"~/docs\" = \"read\"\n\"/srv/out\" = \"write\"\n\"**/*.env\" = \"deny\"\n",
        );
        let result = system.scan();
        let effects: Vec<_> = result
            .findings
            .iter()
            .filter(|f| f.explanation.key.starts_with("finding.codex.profile."))
            .map(|f| (f.target.raw.as_str(), f.effect))
            .collect();
        assert!(effects.contains(&("~/docs", RuleEffect::Allow)));
        assert!(effects.contains(&("/srv/out", RuleEffect::Allow)));
        // A glob without an absolute base cannot be placed and is reported.
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.message.key == "diagnostic.codex.relativePath")
        );
        assert_eq!(find(&result, "finding.codex.tmpdir").len(), 1);
    }

    #[test]
    fn mcp_secrets_and_app_state_extras_are_not_output() {
        let system = System::new("codex-privacy");
        system
            .write(
                "/home/alice/.codex/config.toml",
                "[mcp_servers.files]\ncommand = \"npx\"\nargs = [\"-y\", \"@modelcontextprotocol/server-filesystem\", \"/srv/data\"]\n\
                 [mcp_servers.files.env]\nAPI_TOKEN = \"secret-env-value\"\n\
                 [mcp_servers.remote]\nurl = \"https://mcp.example.com\"\nhttp_headers = { Authorization = \"Bearer secret-header\" }\n",
            )
            .write(
                "/home/alice/.codex/.codex-global-state.json",
                r#"{"local-projects":{"p1":{"rootPaths":["/srv/app"]}},"electron-main-window-bounds":{"x":"secret-state"}}"#,
            )
            .write("/home/alice/.codex/auth.json", r#"{"token":"secret-auth"}"#);
        let result = system.scan();
        let json = serde_json::to_string(&result).unwrap();
        // The message names the index only, never the value.
        for (index, secret) in [
            "secret-env-value",
            "secret-header",
            "secret-state",
            "secret-auth",
        ]
        .iter()
        .enumerate()
        {
            assert!(!json.contains(secret), "test secret #{index} leaked");
        }
        assert_eq!(find(&result, "finding.mcpFilesystemPath").len(), 1);
        assert_eq!(
            find(&result, "finding.codex.workingDirectory.read").len(),
            1
        );
    }

    #[test]
    fn app_mode_applies_to_folders_the_app_uses() {
        let system = System::new("codex-app-mode");
        system
            .write(
                "/home/alice/.codex/config.toml",
                "sandbox_mode = \"read-only\"\n[projects.\"/srv/listed\"]\ntrust_level = \"trusted\"\n",
            )
            .write(
                "/home/alice/.codex/.codex-global-state.json",
                r#"{"local-projects":{"p1":{"rootPaths":["/srv/app"]}},
                    "electron-persisted-atom-state":{"agent-mode-by-host-id":{"local":"full-access"}}}"#,
            );
        let result = system.scan();
        let listed = find(&result, "finding.codex.workingDirectory.read");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].target.raw, "/srv/listed");
        let app = find(&result, "finding.codex.workingDirectory.full");
        assert_eq!(
            app[0].explanation.args["mode"],
            "danger-full-access (app: full-access)"
        );
        let full = find(&result, "finding.codex.fullAccess");
        assert_eq!(full[0].source.config_kind, "codex/app-state");
        assert!(result.summary.iter().any(|s| s.key == "appMode"));
    }

    #[test]
    fn custom_and_unknown_app_modes_fall_back_to_the_configuration() {
        for (mode, diagnostics) in [("custom", 0), ("something-new", 1)] {
            let system = System::new("codex-app-custom");
            system.write(
                "/home/alice/.codex/.codex-global-state.json",
                &format!(
                    r#"{{"local-projects":{{"p1":{{"rootPaths":["/srv/app"]}}}},
                        "electron-persisted-atom-state":{{"agent-mode-by-host-id":{{"local":"{mode}"}}}}}}"#
                ),
            );
            let result = system.scan();
            assert_eq!(
                find(&result, "finding.codex.workingDirectory.read").len(),
                1
            );
            let unknown = result
                .diagnostics
                .iter()
                .filter(|d| d.message.key == "diagnostic.codex.unknownAppMode")
                .count();
            assert_eq!(unknown, diagnostics, "{mode}");
        }
    }
}
