//! Evaluating settings files: permission rules, additional directories,
//! permission modes, sandbox and hooks.

use serde_json::Value;

use super::projects::Project;
use super::rules::{Anchors, PathTool, Rule, parse_rule, resolve_rule_path};
use super::{Scan, unexpected_type};
use crate::analysis::path::TargetPath;
use crate::analysis::resolve::resolve_path;
use crate::model::{
    AccessKind, Confidence, Diagnostic, DiagnosticCode, DiagnosticLevel, Finding, FindingScope,
    FindingStatus, FindingTarget, IneffectiveReason, Message, PatternKind, RuleEffect, Severity,
    SourceDocumentation, SummaryEntry, Warning, WarningKind,
};
use crate::platform::OperatingSystem;

/// Settings levels, highest precedence first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Level {
    Managed,
    ProjectLocal,
    Project,
    User,
}

impl Level {
    pub fn scope(self) -> FindingScope {
        match self {
            Self::Managed => FindingScope::Managed,
            Self::ProjectLocal => FindingScope::ProjectLocal,
            Self::Project => FindingScope::Project,
            Self::User => FindingScope::User,
        }
    }

    pub fn is_project(self) -> bool {
        matches!(self, Self::Project | Self::ProjectLocal)
    }

    fn config_kind(self) -> &'static str {
        match self {
            Self::Managed => "managed-settings",
            _ => "settings",
        }
    }
}

pub(crate) struct SettingsFile {
    pub level: Level,
    pub path: TargetPath,
    pub value: Value,
    /// Index into the project list for project-level files.
    pub project: Option<usize>,
}

impl SettingsFile {
    pub fn pointer(&self, pointer: &str) -> Option<&Value> {
        self.value.pointer(pointer)
    }

    /// The strings of an array setting; reports other types.
    pub fn strings(&self, scan: &mut Scan, pointer: &str) -> Vec<(usize, String)> {
        let Some(value) = self.pointer(pointer) else {
            return Vec::new();
        };
        let Some(items) = value.as_array() else {
            scan.diagnostic(unexpected_type(&self.path, pointer, "an array of strings"));
            return Vec::new();
        };
        let mut strings = Vec::new();
        for (i, item) in items.iter().enumerate() {
            match item.as_str() {
                Some(s) => strings.push((i, s.to_string())),
                None => scan.diagnostic(unexpected_type(
                    &self.path,
                    &format!("{pointer}/{i}"),
                    "a string",
                )),
            }
        }
        strings
    }
}

/// Settings files that apply in one situation, highest precedence first.
pub(crate) struct View<'a> {
    files: Vec<&'a SettingsFile>,
}

impl<'a> View<'a> {
    /// Managed and user settings: what applies everywhere.
    pub fn global(files: &'a [SettingsFile]) -> Self {
        Self::new(files.iter().filter(|f| !f.level.is_project()))
    }

    /// Everything that applies in one project.
    pub fn project(files: &'a [SettingsFile], project: usize) -> Self {
        Self::new(
            files
                .iter()
                .filter(|f| !f.level.is_project() || f.project == Some(project)),
        )
    }

    fn new(files: impl Iterator<Item = &'a SettingsFile>) -> Self {
        let mut files: Vec<&SettingsFile> = files.collect();
        // Stable: drop-ins keep their order within the managed level.
        files.sort_by_key(|f| f.level);
        Self { files }
    }

    pub fn files(&self) -> &[&'a SettingsFile] {
        &self.files
    }

    /// The value from the highest level that sets it.
    pub fn get(&self, pointer: &str) -> Option<(&'a SettingsFile, &'a Value)> {
        self.files
            .iter()
            .find_map(|f| f.pointer(pointer).map(|value| (*f, value)))
    }

    pub fn get_bool(&self, pointer: &str) -> Option<bool> {
        self.get(pointer).and_then(|(_, v)| v.as_bool())
    }

    pub fn any_true(&self, pointer: &str) -> bool {
        self.files
            .iter()
            .any(|f| f.pointer(pointer) == Some(&Value::Bool(true)))
    }

    pub fn any_disabled(&self, pointer: &str) -> bool {
        self.files
            .iter()
            .any(|f| f.pointer(pointer).and_then(Value::as_str) == Some("disable"))
    }

    pub fn managed_rules_only(&self) -> bool {
        self.files.iter().any(|f| {
            f.level == Level::Managed
                && f.pointer("/allowManagedPermissionRulesOnly") == Some(&Value::Bool(true))
        })
    }

    /// The effective permission mode and the file that sets it. `auto` and
    /// `bypassPermissions` have no effect from project files or when they
    /// are disabled by `disableAutoMode`/`disableBypassPermissionsMode`.
    pub fn mode(&self) -> Option<(&'a SettingsFile, String)> {
        self.files.iter().find_map(|f| {
            let mode = f.pointer("/permissions/defaultMode")?.as_str()?;
            let ignored = match mode {
                "bypassPermissions" => {
                    f.level.is_project()
                        || self.any_disabled("/permissions/disableBypassPermissionsMode")
                }
                "auto" => f.level.is_project() || self.any_disabled("/permissions/disableAutoMode"),
                _ => false,
            };
            (!ignored).then(|| (*f, mode.to_string()))
        })
    }

    /// Whether the sandbox is effectively on for shell commands.
    pub fn sandbox(&self, os: OperatingSystem) -> SandboxState {
        if self.get_bool("/sandbox/enabled") != Some(true) {
            SandboxState::Disabled
        } else if os == OperatingSystem::Windows {
            SandboxState::Unsupported
        } else if self.files.iter().any(|f| {
            !f.level.is_project()
                && f.pointer("/sandbox/filesystem/disabled") == Some(&Value::Bool(true))
        }) {
            SandboxState::FilesystemDisabled
        } else {
            SandboxState::Enabled
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SandboxState {
    Enabled,
    Disabled,
    /// Enabled in the settings, but there is no sandbox on native Windows.
    Unsupported,
    /// Enabled, but filesystem isolation is turned off.
    FilesystemDisabled,
}

const PERMISSION_KEYS: &[&str] = &[
    "additionalDirectories",
    "allow",
    "ask",
    "blockReadsOutsideWorkingDirectories",
    "defaultMode",
    "deny",
    "disableAutoMode",
    "disableBypassPermissionsMode",
];

const SANDBOX_KEYS: &[&str] = &[
    "allowAppleEvents",
    "allowNestedUserNamespaces",
    "allowUnsandboxedCommands",
    "autoAllowBashIfSandboxed",
    "bwrapPath",
    "credentials",
    "enableWeakerNestedSandbox",
    "enableWeakerNetworkIsolation",
    "enabled",
    "excludedCommands",
    "failIfUnavailable",
    "filesystem",
    "ignoreViolations",
    "network",
    "ripgrep",
    "socatPath",
];

const SANDBOX_FILESYSTEM_KEYS: &[&str] = &[
    "allowManagedReadPathsOnly",
    "allowRead",
    "allowWrite",
    "denyRead",
    "denyWrite",
    "disabled",
];

/// Settings that only take effect in managed settings (relevant subset).
const MANAGED_ONLY: &[&str] = &[
    "/allowManagedHooksOnly",
    "/allowManagedMcpServersOnly",
    "/allowManagedPermissionRulesOnly",
    "/managedMcpServers",
    "/sandbox/bwrapPath",
    "/sandbox/filesystem/allowManagedReadPathsOnly",
    "/sandbox/network/allowManagedDomainsOnly",
    "/sandbox/socatPath",
];

pub(crate) fn evaluate(scan: &mut Scan, files: &[SettingsFile], projects: &[Project]) {
    let global = View::global(files);
    let managed_rules_only = global.managed_rules_only();

    for file in files {
        check_keys(scan, file);
        let view = match file.project {
            Some(project) => View::project(files, project),
            None => View::global(files),
        };
        let project = file.project.map(|i| &projects[i]);
        let mode = view.mode().map(|(_, mode)| mode);
        rule_findings(scan, file, project, managed_rules_only);
        directory_findings(scan, file, project, mode.as_deref());
        sandbox_path_findings(scan, file, project, view.sandbox(scan.os()));
    }

    mode_findings(scan, &global);
    let global_shell = shell_state(scan.os(), &global, projects, managed_rules_only);
    shell_finding(scan, &global_shell, None);
    for (index, project) in projects.iter().enumerate() {
        let view = View::project(files, index);
        let state = shell_state(scan.os(), &view, projects, managed_rules_only);
        if state != global_shell {
            shell_finding(scan, &state, Some(project));
        }
    }
    summary(scan, files, &global, &global_shell);
}

fn check_keys(scan: &mut Scan, file: &SettingsFile) {
    for (pointer, known) in [
        ("/permissions", PERMISSION_KEYS),
        ("/sandbox", SANDBOX_KEYS),
        ("/sandbox/filesystem", SANDBOX_FILESYSTEM_KEYS),
    ] {
        let Some(value) = file.pointer(pointer) else {
            continue;
        };
        let Some(object) = value.as_object() else {
            scan.diagnostic(unexpected_type(&file.path, pointer, "an object"));
            continue;
        };
        for key in object.keys().filter(|key| !known.contains(&key.as_str())) {
            let key_pointer = format!("{pointer}/{key}");
            scan.diagnostic(
                Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticCode::UnknownKey,
                    Message::new("diagnostic.unknownKey")
                        .arg("key", key_pointer.trim_start_matches('/').replace('/', ".")),
                )
                .file(file.path.to_path_buf())
                .pointer(key_pointer),
            );
        }
    }
    if file.level != Level::Managed {
        for pointer in MANAGED_ONLY {
            if file.pointer(pointer).is_some() {
                scan.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Warning,
                        DiagnosticCode::IgnoredSetting,
                        Message::new("diagnostic.managedOnlySetting")
                            .arg("key", pointer.trim_start_matches('/').replace('/', ".")),
                    )
                    .file(file.path.to_path_buf())
                    .pointer(*pointer),
                );
            }
        }
    }
    if file.level.is_project()
        && let Some(mode @ ("auto" | "bypassPermissions")) = file
            .pointer("/permissions/defaultMode")
            .and_then(Value::as_str)
    {
        scan.diagnostic(
            Diagnostic::new(
                DiagnosticLevel::Warning,
                DiagnosticCode::IgnoredSetting,
                Message::new("diagnostic.scopeNotAllowed")
                    .arg("key", "permissions.defaultMode")
                    .arg("value", mode),
            )
            .file(file.path.to_path_buf())
            .pointer("/permissions/defaultMode"),
        );
    }
}

/// Base for `/path` rules of a file.
fn settings_anchor(
    scan: &Scan,
    file: &SettingsFile,
    project: Option<&Project>,
) -> Option<TargetPath> {
    match file.level {
        Level::Managed => Some(scan.locations.managed_dir.clone()),
        Level::User => Some(scan.locations.config_dir.clone()),
        Level::Project | Level::ProjectLocal => project.map(|p| p.path.clone()),
    }
}

fn trust_warning() -> Warning {
    Warning::new(
        WarningKind::NotEffective {
            reason: IneffectiveReason::TrustNotAccepted,
        },
        Severity::Info,
        Message::new("warning.trustNotAccepted"),
    )
}

fn rule_findings(
    scan: &mut Scan,
    file: &SettingsFile,
    project: Option<&Project>,
    managed_rules_only: bool,
) {
    let home = scan.locations.home.clone();
    let settings_dir = settings_anchor(scan, file, project);
    let working_dir = project.map(|p| p.path.clone());
    let anchors = Anchors {
        home: &home,
        settings: settings_dir.as_ref(),
        working_directory: working_dir.as_ref(),
    };

    for (list, effect) in [
        ("allow", RuleEffect::Allow),
        ("ask", RuleEffect::Ask),
        ("deny", RuleEffect::Deny),
    ] {
        let pointer = format!("/permissions/{list}");
        for (i, rule) in file.strings(scan, &pointer) {
            let item_pointer = format!("{pointer}/{i}");
            match parse_rule(&rule) {
                Rule::Path { tool, spec } => {
                    path_rule_finding(
                        scan,
                        file,
                        project,
                        (&rule, tool, spec),
                        (list, effect),
                        &anchors,
                        &item_pointer,
                        managed_rules_only,
                    );
                }
                Rule::Ineffective { tool } => scan.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Warning,
                        DiagnosticCode::IneffectiveRule,
                        Message::new("diagnostic.ineffectiveRule")
                            .arg("rule", &rule)
                            .arg("tool", tool),
                    )
                    .file(file.path.to_path_buf())
                    .pointer(&item_pointer),
                ),
                Rule::Unparsable => scan.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Warning,
                        DiagnosticCode::UnparsableRule,
                        Message::new("diagnostic.unparsableRule").arg("rule", &rule),
                    )
                    .file(file.path.to_path_buf())
                    .pointer(&item_pointer),
                ),
                Rule::WholeTool(_) | Rule::Other => {}
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn path_rule_finding(
    scan: &mut Scan,
    file: &SettingsFile,
    project: Option<&Project>,
    (rule, tool, spec): (&str, PathTool, &str),
    (list, effect): (&str, RuleEffect),
    anchors: &Anchors,
    pointer: &str,
    managed_rules_only: bool,
) {
    let resolved = match resolve_rule_path(spec, anchors) {
        Ok(resolved) => resolved,
        Err(error) => {
            scan.diagnostic(
                Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticCode::UnparsableRule,
                    Message::new("diagnostic.unresolvedPath")
                        .arg("path", spec)
                        .arg("error", error),
                )
                .file(file.path.to_path_buf())
                .pointer(pointer),
            );
            return;
        }
    };
    let target = match &resolved.base {
        Some(base) => FindingTarget::path(rule, resolved.kind, base.to_path_buf()),
        None => FindingTarget::unresolved(rule, resolved.kind),
    };
    let (effect, explanation) = if resolved.negated {
        (
            RuleEffect::NotApplicable,
            Message::new("finding.rule.negation").arg("effect", list),
        )
    } else {
        (
            effect,
            Message::new(&format!("finding.rule.{}.{list}", tool.key())),
        )
    };
    let source = scan
        .source(
            file.level.config_kind(),
            SourceDocumentation::Documented,
            &file.path,
            pointer,
        )
        .raw_value(rule)
        .anchor(resolved.anchor.clone());
    let mut builder = Finding::builder(&scan.meta, file.level.scope(), target, source)
        .effect(effect)
        .access(tool.access())
        .status(FindingStatus::Configured)
        .confidence(Confidence::High)
        .explanation(explanation);
    if resolved.likely_misanchored {
        let anchor = anchors
            .settings
            .map(ToString::to_string)
            .unwrap_or_else(|| "the working directory".into());
        builder = builder.warning(Warning::new(
            WarningKind::LikelyMisconfigured,
            Severity::Medium,
            Message::new("warning.likelyMisanchored")
                .arg("rule", rule)
                .arg("anchor", anchor),
        ));
    }
    if effect == RuleEffect::Allow && file.level.is_project() && !project.is_some_and(|p| p.trusted)
    {
        builder = builder.warning(trust_warning());
    }
    if managed_rules_only && file.level != Level::Managed {
        builder = builder.warning(Warning::new(
            WarningKind::NotEffective {
                reason: IneffectiveReason::ManagedRulesOnly,
            },
            Severity::Info,
            Message::new("warning.managedRulesOnly"),
        ));
    }
    scan.finding(builder.build());
}

fn directory_findings(
    scan: &mut Scan,
    file: &SettingsFile,
    project: Option<&Project>,
    mode: Option<&str>,
) {
    let access = match mode {
        Some("acceptEdits" | "bypassPermissions") => AccessKind::ReadWrite,
        _ => AccessKind::Read,
    };
    let home = scan.locations.home.clone();
    let pointer = "/permissions/additionalDirectories";
    for (i, raw) in file.strings(scan, pointer) {
        let item_pointer = format!("{pointer}/{i}");
        let base = project.map(|p| &p.path);
        let path = match resolve_path(&raw, &home, base) {
            Ok(path) => path,
            Err(error) => {
                scan.diagnostic(
                    Diagnostic::new(
                        DiagnosticLevel::Warning,
                        DiagnosticCode::UnparsableRule,
                        Message::new("diagnostic.unresolvedPath")
                            .arg("path", &raw)
                            .arg("error", error),
                    )
                    .file(file.path.to_path_buf())
                    .pointer(&item_pointer),
                );
                continue;
            }
        };
        let source = scan
            .source(
                file.level.config_kind(),
                SourceDocumentation::Documented,
                &file.path,
                &item_pointer,
            )
            .raw_value(&raw);
        let mut builder = Finding::builder(
            &scan.meta,
            file.level.scope(),
            FindingTarget::path(&raw, PatternKind::Directory, path.to_path_buf()),
            source,
        )
        .effect(RuleEffect::Allow)
        .access(access)
        .status(FindingStatus::Configured)
        .confidence(Confidence::High)
        .explanation(Message::new("finding.additionalDirectory"));
        if file.level.is_project() && !project.is_some_and(|p| p.trusted) {
            builder = builder.warning(trust_warning());
        }
        scan.finding(builder.build());
    }
}

/// `sandbox.filesystem.*` path lists. Paths use plain conventions: `/`
/// absolute, `~/` home, anything else relative to the project (project
/// settings) or `~/.claude` (user settings).
fn sandbox_path_findings(
    scan: &mut Scan,
    file: &SettingsFile,
    project: Option<&Project>,
    sandbox: SandboxState,
) {
    let home = scan.locations.home.clone();
    let relative_base = match file.level {
        Level::User => Some(scan.locations.config_dir.clone()),
        Level::Project | Level::ProjectLocal => project.map(|p| p.path.clone()),
        Level::Managed => None,
    };
    for (key, effect, access, status) in [
        (
            "allowWrite",
            RuleEffect::Allow,
            AccessKind::Write,
            FindingStatus::Potential,
        ),
        (
            "denyWrite",
            RuleEffect::Deny,
            AccessKind::Write,
            FindingStatus::Configured,
        ),
        (
            "denyRead",
            RuleEffect::Deny,
            AccessKind::Read,
            FindingStatus::Configured,
        ),
        // An exception inside a denyRead region, not a grant of its own.
        (
            "allowRead",
            RuleEffect::NotApplicable,
            AccessKind::Read,
            FindingStatus::Configured,
        ),
    ] {
        let pointer = format!("/sandbox/filesystem/{key}");
        for (i, raw) in file.strings(scan, &pointer) {
            let item_pointer = format!("{pointer}/{i}");
            let resolved = if let Some(absolute) = raw.strip_prefix("//") {
                super::rules::absolute_posix(scan.os(), absolute).map(Some)
            } else {
                match resolve_path(&raw, &home, relative_base.as_ref()) {
                    Ok(path) => Ok(Some(path)),
                    Err(crate::analysis::path::PathError::RelativeWithoutBase(_)) => Ok(None),
                    Err(error) => Err(error),
                }
            };
            let target = match resolved {
                Ok(Some(path)) => {
                    FindingTarget::path(&raw, PatternKind::Directory, path.to_path_buf())
                }
                Ok(None) => FindingTarget::unresolved(&raw, PatternKind::Directory),
                Err(error) => {
                    scan.diagnostic(
                        Diagnostic::new(
                            DiagnosticLevel::Warning,
                            DiagnosticCode::UnparsableRule,
                            Message::new("diagnostic.unresolvedPath")
                                .arg("path", &raw)
                                .arg("error", error),
                        )
                        .file(file.path.to_path_buf())
                        .pointer(&item_pointer),
                    );
                    continue;
                }
            };
            let source = scan
                .source(
                    file.level.config_kind(),
                    SourceDocumentation::Documented,
                    &file.path,
                    &item_pointer,
                )
                .raw_value(&raw);
            let mut builder = Finding::builder(&scan.meta, file.level.scope(), target, source)
                .effect(effect)
                .access(access)
                .status(status)
                .confidence(Confidence::High)
                .explanation(Message::new(&format!("finding.sandbox.{key}")));
            match sandbox {
                SandboxState::Enabled => {}
                SandboxState::Unsupported => {
                    builder = builder.warning(Warning::new(
                        WarningKind::NotEffective {
                            reason: IneffectiveReason::PlatformUnsupported,
                        },
                        Severity::Info,
                        Message::new("warning.sandboxUnsupported"),
                    ));
                }
                SandboxState::Disabled | SandboxState::FilesystemDisabled => {
                    builder = builder.warning(Warning::new(
                        WarningKind::NotEffective {
                            reason: IneffectiveReason::SandboxDisabled,
                        },
                        Severity::Info,
                        Message::new("warning.sandboxDisabled"),
                    ));
                }
            }
            scan.finding(builder.build());
        }
    }
}

/// A finding for `bypassPermissions` or `auto` as effective default mode.
fn mode_findings(scan: &mut Scan, global: &View) {
    let Some((file, mode)) = global.mode() else {
        return;
    };
    let severity = match mode.as_str() {
        "bypassPermissions" => Severity::High,
        "auto" => Severity::Medium,
        _ => return,
    };
    let raw = format!("permissions.defaultMode: {mode}");
    let source = scan
        .source(
            file.level.config_kind(),
            SourceDocumentation::Documented,
            &file.path,
            "/permissions/defaultMode",
        )
        .raw_value(&mode);
    let finding = Finding::builder(
        &scan.meta,
        file.level.scope(),
        FindingTarget::unresolved(raw, PatternKind::Everything),
        source,
    )
    .effect(RuleEffect::Allow)
    .access(AccessKind::ReadWrite)
    .status(FindingStatus::Configured)
    .confidence(Confidence::High)
    .explanation(Message::new(&format!("finding.mode.{mode}")))
    .warning(Warning::new(
        WarningKind::DangerousMode,
        severity,
        Message::new("warning.dangerousMode").arg("mode", &mode),
    ))
    .build();
    scan.finding(finding);
}

/// How shell commands can reach the filesystem in one view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ShellState {
    sandbox: SandboxState,
    /// An allow rule for the whole Bash tool.
    allowed_by: Option<String>,
    /// The Bash tool is denied entirely.
    denied: bool,
    excluded_commands: Vec<String>,
    unsandboxed_retry: bool,
}

fn shell_state(
    os: OperatingSystem,
    view: &View,
    projects: &[Project],
    managed_rules_only: bool,
) -> ShellState {
    let mut allowed_by = None;
    let mut denied = false;
    let mut excluded_commands = Vec::new();
    for file in view.files() {
        let rules_apply = !managed_rules_only || file.level == Level::Managed;
        let trusted = !file.level.is_project() || file.project.is_some_and(|p| projects[p].trusted);
        for (list, values) in [
            ("allow", "/permissions/allow"),
            ("deny", "/permissions/deny"),
        ] {
            let rules = file.pointer(values).and_then(Value::as_array);
            for rule in rules.into_iter().flatten().filter_map(Value::as_str) {
                if parse_rule(rule) == Rule::WholeTool("Bash") && rules_apply {
                    match list {
                        "deny" => denied = true,
                        _ if trusted => {
                            allowed_by.get_or_insert_with(|| rule.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        if let Some(commands) = file
            .pointer("/sandbox/excludedCommands")
            .and_then(Value::as_array)
        {
            for command in commands.iter().filter_map(Value::as_str) {
                if !excluded_commands.iter().any(|c| c == command) {
                    excluded_commands.push(command.to_string());
                }
            }
        }
    }
    ShellState {
        sandbox: view.sandbox(os),
        allowed_by,
        denied,
        excluded_commands,
        unsandboxed_retry: view.get_bool("/sandbox/allowUnsandboxedCommands") != Some(false),
    }
}

fn shell_finding(scan: &mut Scan, state: &ShellState, project: Option<&Project>) {
    if state.denied {
        return;
    }
    let (raw, scope) = match project {
        Some(project) => (format!("Bash ({})", project.path), FindingScope::Project),
        None => ("Bash".to_string(), FindingScope::OperatingSystem),
    };
    let sandboxed = state.sandbox == SandboxState::Enabled;
    let mut builder = Finding::builder(
        &scan.meta,
        scope,
        FindingTarget::unresolved(raw, PatternKind::Everything),
        scan.derived_source(),
    )
    .effect(RuleEffect::Allow)
    .status(FindingStatus::Potential)
    .confidence(Confidence::Medium);
    builder = if sandboxed {
        builder
            .access(AccessKind::Read)
            .explanation(Message::new("finding.sandboxedShell"))
    } else {
        builder
            .access(AccessKind::ReadWrite)
            .explanation(Message::new("finding.unsandboxedShell"))
            .warning(Warning::new(
                WarningKind::UnsandboxedShell,
                Severity::High,
                Message::new("warning.unsandboxedShell"),
            ))
    };
    if state.sandbox == SandboxState::Unsupported {
        builder = builder.warning(Warning::new(
            WarningKind::NotEffective {
                reason: IneffectiveReason::PlatformUnsupported,
            },
            Severity::Info,
            Message::new("warning.sandboxUnsupported"),
        ));
    }
    if let Some(rule) = &state.allowed_by {
        builder = builder.warning(Warning::new(
            WarningKind::DangerousMode,
            Severity::Medium,
            Message::new("warning.shellAllowedWithoutPrompt").arg("rule", rule),
        ));
    }
    if sandboxed {
        for command in &state.excluded_commands {
            builder = builder.warning(Warning::new(
                WarningKind::UnsandboxedShell,
                Severity::Medium,
                Message::new("warning.sandboxExcludedCommand").arg("command", command),
            ));
        }
        if state.unsandboxed_retry {
            builder = builder.warning(Warning::new(
                WarningKind::UnsandboxedShell,
                Severity::Low,
                Message::new("warning.unsandboxedRetry"),
            ));
        }
    }
    scan.finding(builder.build());
}

fn summary(scan: &mut Scan, files: &[SettingsFile], global: &View, shell: &ShellState) {
    let mode = global
        .mode()
        .map(|(_, mode)| mode)
        .unwrap_or_else(|| "default".into());
    let sandbox = match shell.sandbox {
        SandboxState::Enabled => Message::new("value.enabled"),
        SandboxState::Disabled | SandboxState::FilesystemDisabled => Message::new("value.disabled"),
        SandboxState::Unsupported => Message::new("value.notSupported"),
    };
    let shell_value = if shell.denied {
        Message::new("value.blocked")
    } else if shell.allowed_by.is_some() {
        Message::new("value.allowedWithoutPrompt")
    } else {
        Message::new("value.withPrompt")
    };
    let reads_outside = if global.any_true("/permissions/blockReadsOutsideWorkingDirectories") {
        Message::new("value.blocked")
    } else {
        Message::new("value.withPrompt")
    };
    let hooks: usize = files.iter().map(|f| count_hooks(f.pointer("/hooks"))).sum();
    let managed = files.iter().filter(|f| f.level == Level::Managed).count();
    let count = |n: usize| {
        if n == 0 {
            Message::new("value.none")
        } else {
            Message::raw(n)
        }
    };
    let entries = [
        ("permissionMode", Message::raw(mode)),
        ("sandbox", sandbox),
        ("shell", shell_value),
        ("readsOutsideWorkingDirectories", reads_outside),
        ("hooks", count(hooks)),
        ("managedSettings", count(managed)),
    ];
    for (key, value) in entries {
        scan.out.summary.push(SummaryEntry::new(
            key,
            Message::new(&format!("summary.{key}")),
            value,
        ));
    }
}

/// Number of hook commands: `hooks.<event>[].hooks[]`.
fn count_hooks(hooks: Option<&Value>) -> usize {
    let Some(events) = hooks.and_then(Value::as_object) else {
        return 0;
    };
    events
        .values()
        .filter_map(Value::as_array)
        .flatten()
        .map(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .map_or(1, Vec::len)
        })
        .sum()
}
