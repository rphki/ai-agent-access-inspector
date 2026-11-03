//! Sandbox mode, permission profiles and managed requirements: which areas
//! Codex's commands may read and write.

use serde_json::{Map, Value};

use super::projects::{AppMode, Origin as ProjectOrigin, Project};
use super::{Layer, Requirements, Scan, View, pointer_key, unexpected_type};
use crate::analysis::path::TargetPath;
use crate::analysis::pattern::split_pattern;
use crate::analysis::resolve::resolve_path;
use crate::model::{
    AccessKind, Confidence, Diagnostic, DiagnosticCode, DiagnosticLevel, Finding, FindingScope,
    FindingSource, FindingStatus, FindingTarget, IneffectiveReason, Message, PatternKind,
    RuleEffect, Severity, SourceDocumentation, SummaryEntry, Warning, WarningKind,
};
use crate::platform::OperatingSystem;
use crate::read;

/// Directories that stay read-only inside a writable root.
const PROTECTED: &[&str] = &[".git", ".agents", ".codex", ".aws"];

/// Longest `extends` chain that is followed.
const MAX_EXTENDS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Mode {
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}

impl Mode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "read-only" => Some(Mode::ReadOnly),
            "workspace-write" => Some(Mode::WorkspaceWrite),
            "danger-full-access" => Some(Mode::FullAccess),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::ReadOnly => "read-only",
            Mode::WorkspaceWrite => "workspace-write",
            Mode::FullAccess => "danger-full-access",
        }
    }

    fn builtin_profile(name: &str) -> Option<Self> {
        match name {
            ":read-only" => Some(Mode::ReadOnly),
            ":workspace" => Some(Mode::WorkspaceWrite),
            ":danger-full-access" => Some(Mode::FullAccess),
            _ => None,
        }
    }
}

/// Where the effective mode comes from.
#[derive(Debug, Clone)]
pub(crate) enum Origin {
    /// `sandbox_mode` in a configuration file.
    SandboxMode {
        file: TargetPath,
        scope: FindingScope,
    },
    /// `default_permissions` selects a permission profile.
    Profile {
        file: TargetPath,
        scope: FindingScope,
    },
    /// The permission mode selected in the ChatGPT app (app state).
    App { source: FindingSource },
    /// Nothing configured: Codex's default for the folder.
    Default,
}

/// The effective sandbox of one place.
#[derive(Debug, Clone)]
pub(crate) struct Policy {
    pub mode: Mode,
    /// Mode or profile name as shown to the user.
    pub label: String,
    pub origin: Origin,
    /// Entries of the active custom profile, if any.
    pub profile: Option<Profile>,
    /// A configured value the requirements do not allow.
    pub rejected: Option<String>,
}

/// A resolved custom permission profile.
#[derive(Debug, Clone)]
pub(crate) struct Profile {
    pub name: String,
    /// The built-in profile at the end of the `extends` chain; `None` if
    /// the chain does not end in one, so only the listed entries apply.
    pub base: Option<Mode>,
    pub entries: Vec<ProfileEntry>,
}

#[derive(Debug, Clone)]
pub(crate) struct ProfileEntry {
    pub profile: String,
    /// Path or token as written, e.g. `~/data` or `:tmpdir`.
    pub raw: String,
    /// For `:workspace_roots` entries: the subpath below each root.
    pub subpath: Option<String>,
    pub access: String,
    pub file: TargetPath,
    pub scope: FindingScope,
    pub pointer: String,
}

pub(crate) struct Policies {
    /// One per project, same order as the project list.
    pub projects: Vec<Policy>,
}

pub(crate) fn evaluate(
    scan: &mut Scan,
    layers: &[Layer],
    projects: &[Project],
    requirements: Option<&Requirements>,
    app_mode: Option<&AppMode>,
) -> Policies {
    let global_view = View::global(layers);
    let global = policy(scan, &global_view, None, requirements);
    let configured = global_view
        .get_str("/sandbox_mode")
        .and_then(|(_, mode)| Mode::parse(mode));
    let app_policy = app_mode.and_then(|mode| app_policy(scan, mode, configured, requirements));
    let project_policies: Vec<Policy> = projects
        .iter()
        .enumerate()
        .map(|(index, project)| match &app_policy {
            // Folders the app uses run with the mode selected there.
            Some(app) if project.origin != ProjectOrigin::Config => app.clone(),
            _ => policy(
                scan,
                &View::project(layers, index),
                Some(project),
                requirements,
            ),
        })
        .collect();

    let mut all: Vec<&Policy> = vec![&global];
    all.extend(project_policies.iter());

    let never_asks = global_view.get_str("/approval_policy").map(|(_, p)| p) == Some("never");
    mode_findings(scan, &all, never_asks);
    if scan.os() == OperatingSystem::Windows && global_view.get("/windows/sandbox").is_none() {
        windows_finding(scan);
    }
    temp_findings(scan, &global_view, &all);
    writable_root_findings(scan, layers, &global, &project_policies);
    protected_findings(scan, layers, projects, &project_policies);
    for policy in dedup_profiles(&all) {
        profile_findings(scan, policy, projects);
    }
    if let Some(requirements) = requirements {
        deny_read_findings(scan, requirements);
    }
    scan.out.summary.push(SummaryEntry::new(
        "sandboxMode",
        Message::new("summary.codex.sandboxMode"),
        Message::raw(&global.label),
    ));
    if let Some(app_mode) = app_mode {
        scan.out.summary.push(SummaryEntry::new(
            "appMode",
            Message::new("summary.codex.appMode"),
            Message::raw(&app_mode.value),
        ));
    }
    Policies {
        projects: project_policies,
    }
}

/// The effective policy of one view. Without a configured mode the default
/// is `workspace-write` for folders with a `[projects]` entry (trusted or
/// not) and `read-only` otherwise (`config_toml.rs` in the Codex source).
fn policy(
    scan: &mut Scan,
    view: &View,
    project: Option<&Project>,
    requirements: Option<&Requirements>,
) -> Policy {
    let mut policy = if let Some((layer, value)) = view.get("/sandbox_mode") {
        match value.as_str().and_then(Mode::parse) {
            Some(mode) => {
                if view.get("/default_permissions").is_some() && project.is_none() {
                    scan.diagnostic(
                        Diagnostic::new(
                            DiagnosticLevel::Info,
                            DiagnosticCode::IgnoredSetting,
                            Message::new("diagnostic.codex.profileIgnored"),
                        )
                        .file(layer.path.to_path_buf())
                        .pointer("/sandbox_mode"),
                    );
                }
                Policy {
                    mode,
                    label: mode.name().into(),
                    origin: Origin::SandboxMode {
                        file: layer.path.clone(),
                        scope: layer.level.scope(),
                    },
                    profile: None,
                    rejected: None,
                }
            }
            None => {
                if project.is_none() {
                    scan.diagnostic(unexpected_type(
                        &layer.path,
                        "/sandbox_mode",
                        "read-only, workspace-write or danger-full-access",
                    ));
                }
                default_policy(scan, project)
            }
        }
    } else if let Some((layer, name)) = view.get_str("/default_permissions") {
        profile_policy(scan, view, layer, name, project.is_none())
            .unwrap_or_else(|| default_policy(scan, project))
    } else {
        default_policy(scan, project)
    };
    if let Some(requirements) = requirements {
        constrain(&mut policy, requirements);
    }
    policy
}

/// The app's permission modes (`agent-mode-by-host-id`, values taken from
/// the app bundle): `auto`, `granular` and `guardian-approvals` write in the
/// workspace (`guardian-approvals` follows a configured `read-only`),
/// `custom` uses the configuration, unknown values are reported and ignored.
fn app_policy(
    scan: &mut Scan,
    app_mode: &AppMode,
    global_mode: Option<Mode>,
    requirements: Option<&Requirements>,
) -> Option<Policy> {
    let mode = match app_mode.value.as_str() {
        "read-only" => Mode::ReadOnly,
        "auto" | "granular" => Mode::WorkspaceWrite,
        "guardian-approvals" => {
            if global_mode == Some(Mode::ReadOnly) {
                Mode::ReadOnly
            } else {
                Mode::WorkspaceWrite
            }
        }
        "full-access" => Mode::FullAccess,
        "custom" => return None,
        other => {
            scan.diagnostic(
                Diagnostic::new(
                    DiagnosticLevel::Info,
                    DiagnosticCode::Other,
                    Message::new("diagnostic.codex.unknownAppMode").arg("mode", other),
                )
                .file(scan.locations.app_state().to_path_buf()),
            );
            return None;
        }
    };
    let mut policy = Policy {
        mode,
        label: format!("{} (app: {})", mode.name(), app_mode.value),
        origin: Origin::App {
            source: app_mode.source.clone(),
        },
        profile: None,
        rejected: None,
    };
    if let Some(requirements) = requirements {
        constrain(&mut policy, requirements);
    }
    Some(policy)
}

fn default_policy(scan: &Scan, project: Option<&Project>) -> Policy {
    // On Windows without a sandbox the default profile is always read-only.
    let windows_unsandboxed = scan.os() == OperatingSystem::Windows;
    let mode = match project {
        Some(project) if project.listed && !windows_unsandboxed => Mode::WorkspaceWrite,
        _ => Mode::ReadOnly,
    };
    Policy {
        mode,
        label: mode.name().into(),
        origin: Origin::Default,
        profile: None,
        rejected: None,
    }
}

fn profile_policy(
    scan: &mut Scan,
    view: &View,
    layer: &Layer,
    name: &str,
    report: bool,
) -> Option<Policy> {
    let origin = Origin::Profile {
        file: layer.path.clone(),
        scope: layer.level.scope(),
    };
    if let Some(mode) = Mode::builtin_profile(name) {
        return Some(Policy {
            mode,
            label: name.into(),
            origin,
            profile: None,
            rejected: None,
        });
    }
    let profile = resolve_profile(scan, view, name, report)?;
    Some(Policy {
        mode: profile.base.unwrap_or(Mode::ReadOnly),
        label: name.into(),
        origin,
        profile: Some(profile),
        rejected: None,
    })
}

/// Follows `extends` and collects the filesystem entries, base first.
fn resolve_profile(scan: &mut Scan, view: &View, name: &str, report: bool) -> Option<Profile> {
    let mut chain: Vec<String> = Vec::new();
    let mut current = name.to_string();
    let mut base = None;
    let mut tables: Vec<(String, &Layer, &Map<String, Value>)> = Vec::new();
    loop {
        if let Some(mode) = Mode::builtin_profile(&current) {
            base = Some(mode);
            break;
        }
        if chain.contains(&current) || chain.len() >= MAX_EXTENDS {
            if report {
                scan.diagnostic(Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticCode::Other,
                    Message::new("diagnostic.codex.profileCycle").arg("profile", &current),
                ));
            }
            break;
        }
        let pointer = format!("/permissions/{}", pointer_key(&current));
        let Some((layer, Value::Object(table))) = view.get(&pointer) else {
            if report {
                scan.diagnostic(Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticCode::Other,
                    Message::new("diagnostic.codex.unknownProfile").arg("profile", &current),
                ));
            }
            if chain.is_empty() {
                return None;
            }
            break;
        };
        chain.push(current.clone());
        tables.push((current.clone(), layer, table));
        match table.get("extends").and_then(Value::as_str) {
            Some(parent) => current = parent.to_string(),
            None => break,
        }
    }
    let mut entries = Vec::new();
    // Base first, so that later (more derived) entries come last.
    for (profile, layer, table) in tables.iter().rev() {
        let pointer = format!("/permissions/{}/filesystem", pointer_key(profile));
        let Some(filesystem) = table.get("filesystem").and_then(Value::as_object) else {
            continue;
        };
        for (key, value) in filesystem {
            let key_pointer = format!("{pointer}/{}", pointer_key(key));
            match value {
                Value::String(access) => entries.push(ProfileEntry {
                    profile: profile.clone(),
                    raw: key.clone(),
                    subpath: None,
                    access: access.clone(),
                    file: layer.path.clone(),
                    scope: layer.level.scope(),
                    pointer: key_pointer,
                }),
                Value::Object(subpaths) => {
                    for (subpath, access) in subpaths {
                        let Some(access) = access.as_str() else {
                            continue;
                        };
                        entries.push(ProfileEntry {
                            profile: profile.clone(),
                            raw: key.clone(),
                            subpath: Some(subpath.clone()),
                            access: access.into(),
                            file: layer.path.clone(),
                            scope: layer.level.scope(),
                            pointer: format!("{key_pointer}/{}", pointer_key(subpath)),
                        });
                    }
                }
                // `glob_scan_max_depth` and similar settings.
                _ => {}
            }
        }
        let roots_pointer = format!("/permissions/{}/workspace_roots", pointer_key(profile));
        if let Some(roots) = table.get("workspace_roots").and_then(Value::as_object) {
            for (root, enabled) in roots {
                if enabled.as_bool() == Some(true) {
                    entries.push(ProfileEntry {
                        profile: profile.clone(),
                        raw: root.clone(),
                        subpath: None,
                        access: "workspace".into(),
                        file: layer.path.clone(),
                        scope: layer.level.scope(),
                        pointer: format!("{roots_pointer}/{}", pointer_key(root)),
                    });
                }
            }
        }
    }
    Some(Profile {
        name: name.into(),
        base,
        entries,
    })
}

/// Applies `allowed_sandbox_modes`, `allowed_permission_profiles` and
/// `deny_read`: a value that is not allowed falls back to the most
/// restrictive allowed one.
fn constrain(policy: &mut Policy, requirements: &Requirements) {
    let value = &requirements.value;
    let allowed_modes: Option<Vec<Mode>> = value
        .get("allowed_sandbox_modes")
        .and_then(Value::as_array)
        .map(|modes| {
            modes
                .iter()
                .filter_map(|m| m.as_str().and_then(Mode::parse))
                .collect()
        });
    let deny_read = value.pointer("/permissions/filesystem/deny_read").is_some();
    let allowed = |mode: Mode| {
        allowed_modes
            .as_ref()
            .is_none_or(|modes| modes.contains(&mode))
            && !(deny_read && mode == Mode::FullAccess)
    };
    let profile_allowed = match (&policy.origin, value.get("allowed_permission_profiles")) {
        (Origin::Profile { .. }, Some(Value::Object(profiles))) => {
            profiles.get(&policy.label).and_then(Value::as_bool) == Some(true)
        }
        _ => true,
    };
    if allowed(policy.mode) && profile_allowed {
        return;
    }
    let fallback = [Mode::ReadOnly, Mode::WorkspaceWrite, Mode::FullAccess]
        .into_iter()
        .find(|mode| allowed(*mode))
        .unwrap_or(Mode::ReadOnly);
    policy.rejected = Some(std::mem::replace(&mut policy.label, fallback.name().into()));
    policy.mode = fallback;
    policy.profile = None;
}

pub(crate) fn rejected_warning(policy: &Policy) -> Option<Warning> {
    let rejected = policy.rejected.as_ref()?;
    Some(Warning::new(
        WarningKind::NotEffective {
            reason: IneffectiveReason::OverriddenByHigherScope,
        },
        Severity::Info,
        Message::new("warning.codex.notAllowedByRequirements")
            .arg("value", rejected)
            .arg("fallback", &policy.label),
    ))
}

/// Reading across the computer (every mode with a sandbox) and full access.
fn mode_findings(scan: &mut Scan, policies: &[&Policy], never_asks: bool) {
    let reads_everywhere = policies.iter().any(|p| match &p.profile {
        Some(profile) => profile.base.is_some(),
        None => p.mode != Mode::FullAccess,
    });
    if reads_everywhere {
        let finding = Finding::builder(
            &scan.meta,
            FindingScope::OperatingSystem,
            FindingTarget::unresolved("sandbox: read", PatternKind::Everything),
            scan.derived_source(),
        )
        .effect(RuleEffect::Allow)
        .access(AccessKind::Read)
        .status(FindingStatus::Potential)
        .confidence(Confidence::High)
        .explanation(Message::new("finding.codex.readEverything"))
        .build();
        scan.finding(finding);
    }
    let mut seen: Vec<String> = Vec::new();
    for policy in policies.iter().filter(|p| p.mode == Mode::FullAccess) {
        let (source, scope) = match &policy.origin {
            Origin::SandboxMode { file, scope } => (
                scan.source(
                    "config",
                    SourceDocumentation::Documented,
                    file,
                    "/sandbox_mode",
                ),
                *scope,
            ),
            Origin::Profile { file, scope } => (
                scan.source(
                    "config",
                    SourceDocumentation::Documented,
                    file,
                    "/default_permissions",
                ),
                *scope,
            ),
            Origin::App { source } => (source.clone(), FindingScope::User),
            Origin::Default => (scan.derived_source(), FindingScope::OperatingSystem),
        };
        let key = format!("{:?}{:?}", source.file, source.pointer);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let builder = Finding::builder(
            &scan.meta,
            scope,
            FindingTarget::unresolved(
                format!("sandbox: {}", policy.label),
                PatternKind::Everything,
            ),
            source.raw_value(&policy.label),
        )
        .effect(RuleEffect::Allow)
        .access(AccessKind::ReadWrite)
        .status(FindingStatus::Potential)
        .confidence(Confidence::High)
        .explanation(Message::new("finding.codex.fullAccess").arg("mode", &policy.label))
        .warning(Warning::new(
            WarningKind::DangerousMode,
            Severity::High,
            Message::new("warning.codex.fullAccess"),
        ));
        // The app's full access mode never asks either.
        let builder = if never_asks || matches!(policy.origin, Origin::App { .. }) {
            builder.warning(Warning::new(
                WarningKind::DangerousMode,
                Severity::High,
                Message::new("warning.codex.neverAsks"),
            ))
        } else {
            builder
        };
        scan.finding(builder.build());
    }
}

/// Without `[windows] sandbox`, Codex runs commands on Windows without a
/// sandbox; it may still pick one at runtime, hence medium confidence.
fn windows_finding(scan: &mut Scan) {
    let finding = Finding::builder(
        &scan.meta,
        FindingScope::OperatingSystem,
        FindingTarget::unresolved("windows.sandbox: not set", PatternKind::Everything),
        scan.derived_source(),
    )
    .effect(RuleEffect::Allow)
    .access(AccessKind::ReadWrite)
    .status(FindingStatus::Potential)
    .confidence(Confidence::Medium)
    .explanation(Message::new("finding.codex.windowsUnsandboxed"))
    .warning(Warning::new(
        WarningKind::UnsandboxedShell,
        Severity::High,
        Message::new("warning.codex.windowsUnsandboxed"),
    ))
    .build();
    scan.finding(finding);
}

fn writes_workspace(policy: &Policy) -> bool {
    policy.mode == Mode::WorkspaceWrite
}

/// `$TMPDIR` and `/tmp` in `workspace-write`, unless excluded.
fn temp_findings(scan: &mut Scan, global: &View, policies: &[&Policy]) {
    if !policies.iter().any(|p| writes_workspace(p)) {
        return;
    }
    let mut targets = Vec::new();
    // Codex only uses `$TMPDIR` when it is set.
    if global.get_bool("/sandbox_workspace_write/exclude_tmpdir_env_var") != Some(true)
        && let Some(path) = scan
            .context
            .env_var("TMPDIR")
            .and_then(|tmp| scan.target_path(tmp))
    {
        targets.push(("$TMPDIR", Some(path), "finding.codex.tmpdir"));
    }
    if scan.os() != OperatingSystem::Windows
        && global.get_bool("/sandbox_workspace_write/exclude_slash_tmp") != Some(true)
    {
        targets.push(("/tmp", scan.target_path("/tmp"), "finding.codex.slashTmp"));
    }
    for (raw, path, explanation) in targets {
        let target = match path {
            Some(path) => FindingTarget::path(raw, PatternKind::Directory, path.to_path_buf()),
            None => FindingTarget::unresolved(raw, PatternKind::Directory),
        };
        let finding = Finding::builder(
            &scan.meta,
            FindingScope::OperatingSystem,
            target,
            scan.derived_source(),
        )
        .effect(RuleEffect::Allow)
        .access(AccessKind::Write)
        .status(FindingStatus::Potential)
        .confidence(Confidence::High)
        .explanation(Message::new(explanation))
        .build();
        scan.finding(finding);
    }
}

/// `sandbox_workspace_write.writable_roots` of every file. Relative paths
/// are resolved against the file's directory. Arrays are not merged: a
/// higher layer replaces the list. Inactive unless the mode that applies
/// there is `workspace-write`.
fn writable_root_findings(scan: &mut Scan, layers: &[Layer], global: &Policy, projects: &[Policy]) {
    let pointer = "/sandbox_workspace_write/writable_roots";
    for layer in layers {
        let Some(value) = layer.value.pointer(pointer) else {
            continue;
        };
        let Some(roots) = value.as_array() else {
            scan.diagnostic(unexpected_type(&layer.path, pointer, "an array of paths"));
            continue;
        };
        let policy = layer.project.map_or(global, |p| &projects[p]);
        // A higher layer of the same reach that sets the list replaces it.
        let overridden = layers
            .iter()
            .take_while(|l| !std::ptr::eq(*l, layer))
            .any(|l| {
                l.project == layer.project
                    && l.value
                        .pointer("/sandbox_workspace_write/writable_roots")
                        .is_some()
            });
        for (i, root) in roots.iter().enumerate() {
            let item_pointer = format!("{pointer}/{i}");
            let Some(raw) = root.as_str() else {
                scan.diagnostic(unexpected_type(&layer.path, &item_pointer, "a path"));
                continue;
            };
            let directory = layer.path.parent();
            let Some(target) =
                path_target_in(scan, raw, directory.as_ref(), &layer.path, &item_pointer)
            else {
                continue;
            };
            let source = scan
                .source(
                    layer.level.config_kind(),
                    SourceDocumentation::Documented,
                    &layer.path,
                    &item_pointer,
                )
                .raw_value(raw);
            let mut builder = Finding::builder(&scan.meta, layer.level.scope(), target, source)
                .effect(RuleEffect::Allow)
                .access(AccessKind::ReadWrite)
                .status(FindingStatus::Configured)
                .confidence(Confidence::High)
                .explanation(Message::new("finding.codex.writableRoot"));
            if overridden {
                builder = builder.warning(Warning::new(
                    WarningKind::NotEffective {
                        reason: IneffectiveReason::OverriddenByHigherScope,
                    },
                    Severity::Info,
                    Message::new("warning.codex.writableRootsReplaced"),
                ));
            } else if !writes_workspace(policy) {
                builder = builder.warning(Warning::new(
                    WarningKind::NotEffective {
                        reason: IneffectiveReason::SandboxDisabled,
                    },
                    Severity::Info,
                    Message::new("warning.codex.workspaceWriteOnly").arg("mode", &policy.label),
                ));
            }
            scan.finding(builder.build());
        }
    }
}

/// Existing `.git`, `.agents`, `.codex` and `.aws` inside writable working
/// directories stay read-only.
fn protected_findings(
    scan: &mut Scan,
    layers: &[Layer],
    projects: &[Project],
    policies: &[Policy],
) {
    let mut roots: Vec<TargetPath> = projects
        .iter()
        .zip(policies)
        .filter(|(_, policy)| writes_workspace(policy))
        .map(|(project, _)| project.path.clone())
        .collect();
    for layer in layers {
        let roots_value = layer
            .value
            .pointer("/sandbox_workspace_write/writable_roots")
            .and_then(Value::as_array);
        let directory = layer.path.parent();
        for raw in roots_value.into_iter().flatten().filter_map(Value::as_str) {
            if let Ok(path) = resolve_path(raw, &scan.locations.home, directory.as_ref()) {
                roots.push(path);
            }
        }
    }
    let mut seen: Vec<TargetPath> = Vec::new();
    for root in roots {
        for name in PROTECTED {
            let Ok(path) = root.join(name) else { continue };
            let present = if *name == ".git" {
                read::exists(scan.context, &path.to_string())
            } else {
                read::is_dir(scan.context, &path.to_string())
            };
            if !present || seen.iter().any(|p| p.same_path(&path)) {
                continue;
            }
            seen.push(path.clone());
            let finding = Finding::builder(
                &scan.meta,
                FindingScope::OperatingSystem,
                FindingTarget::path(path.to_string(), PatternKind::Directory, path.to_path_buf()),
                scan.derived_source(),
            )
            .effect(RuleEffect::Deny)
            .access(AccessKind::Write)
            .status(FindingStatus::Configured)
            .confidence(Confidence::High)
            .explanation(Message::new("finding.codex.protectedPath").arg("name", name))
            .build();
            scan.finding(finding);
        }
    }
}

/// Each custom profile once, even if it applies in several projects.
fn dedup_profiles<'a>(policies: &[&'a Policy]) -> Vec<&'a Policy> {
    let mut unique: Vec<&Policy> = Vec::new();
    for policy in policies {
        if let Some(profile) = &policy.profile
            && !unique
                .iter()
                .any(|p| p.profile.as_ref().is_some_and(|q| q.name == profile.name))
        {
            unique.push(policy);
        }
    }
    unique
}

fn profile_findings(scan: &mut Scan, policy: &Policy, projects: &[Project]) {
    let Some(profile) = &policy.profile else {
        return;
    };
    for entry in &profile.entries {
        let (effect, access, explanation) = match entry.access.as_str() {
            "read" => (
                RuleEffect::Allow,
                AccessKind::Read,
                "finding.codex.profile.read",
            ),
            "write" => (
                RuleEffect::Allow,
                AccessKind::ReadWrite,
                "finding.codex.profile.write",
            ),
            "deny" => (
                RuleEffect::Deny,
                AccessKind::ReadWrite,
                "finding.codex.profile.deny",
            ),
            "workspace" => (
                RuleEffect::Allow,
                if profile.base.is_some_and(|b| b >= Mode::WorkspaceWrite) {
                    AccessKind::ReadWrite
                } else {
                    AccessKind::Read
                },
                "finding.codex.profile.workspaceRoot",
            ),
            _ => {
                scan.diagnostic(unexpected_type(
                    &entry.file,
                    &entry.pointer,
                    "read, write or deny",
                ));
                continue;
            }
        };
        let raw = match &entry.subpath {
            Some(subpath) => format!("{} {subpath}", entry.raw),
            None => entry.raw.clone(),
        };
        let targets: Vec<FindingTarget> = match (entry.raw.as_str(), &entry.subpath) {
            (":workspace_roots", subpath) => projects
                .iter()
                .filter_map(|project| {
                    let path = match subpath.as_deref() {
                        None | Some(".") => Some(project.path.clone()),
                        Some(sub) => project.path.join(sub).ok(),
                    }?;
                    Some(pattern_target(&raw, &path.to_string()))
                })
                .collect(),
            (":root", _) => vec![FindingTarget::unresolved(&raw, PatternKind::Everything)],
            (":minimal", _) => vec![FindingTarget::unresolved(&raw, PatternKind::Unparsed)],
            (":tmpdir", _) => vec![match scan
                .context
                .env_var("TMPDIR")
                .and_then(|tmp| scan.target_path(tmp))
            {
                Some(path) => FindingTarget::path(&raw, PatternKind::Directory, path.to_path_buf()),
                None => FindingTarget::unresolved(&raw, PatternKind::Directory),
            }],
            (":slash_tmp", _) => vec![FindingTarget::path(
                &raw,
                PatternKind::Directory,
                std::path::PathBuf::from("/tmp"),
            )],
            (path, None) => match path_target(scan, path, &entry.file, &entry.pointer) {
                Some(target) => vec![target],
                None => continue,
            },
            (path, Some(subpath)) => {
                let joined = format!("{}/{subpath}", path.trim_end_matches('/'));
                match path_target(scan, &joined, &entry.file, &entry.pointer) {
                    Some(target) => vec![target],
                    None => continue,
                }
            }
        };
        for target in targets {
            let source = scan
                .source(
                    "config",
                    SourceDocumentation::Documented,
                    &entry.file,
                    &entry.pointer,
                )
                .raw_value(&raw);
            let mut builder = Finding::builder(&scan.meta, entry.scope, target, source)
                .effect(effect)
                .access(access)
                .status(FindingStatus::Configured)
                .confidence(Confidence::High)
                .explanation(Message::new(explanation).arg("profile", &entry.profile));
            if let Some(warning) = rejected_warning(policy) {
                builder = builder.warning(warning);
            }
            scan.finding(builder.build());
        }
    }
}

/// `requirements.toml` → `[permissions.filesystem] deny_read`.
fn deny_read_findings(scan: &mut Scan, requirements: &Requirements) {
    let pointer = "/permissions/filesystem/deny_read";
    let Some(value) = requirements.value.pointer(pointer) else {
        return;
    };
    let Some(paths) = value.as_array() else {
        scan.diagnostic(unexpected_type(
            &requirements.path,
            pointer,
            "an array of paths",
        ));
        return;
    };
    for (i, raw) in paths.iter().enumerate() {
        let item_pointer = format!("{pointer}/{i}");
        let Some(raw) = raw.as_str() else {
            scan.diagnostic(unexpected_type(&requirements.path, &item_pointer, "a path"));
            continue;
        };
        let Some(target) = path_target(scan, raw, &requirements.path, &item_pointer) else {
            continue;
        };
        let source = scan
            .source(
                "requirements",
                SourceDocumentation::Documented,
                &requirements.path,
                &item_pointer,
            )
            .raw_value(raw);
        let mut builder = Finding::builder(&scan.meta, FindingScope::Managed, target, source)
            .effect(RuleEffect::Deny)
            .access(AccessKind::Read)
            .status(FindingStatus::Configured)
            .confidence(Confidence::High)
            .explanation(Message::new("finding.codex.denyRead"));
        if scan.os() == OperatingSystem::Windows {
            builder = builder.warning(Warning::new(
                WarningKind::NotEffective {
                    reason: IneffectiveReason::PlatformUnsupported,
                },
                Severity::Info,
                Message::new("warning.codex.denyReadUnelevated"),
            ));
        }
        scan.finding(builder.build());
    }
}

/// A path or glob from the configuration as finding target. Paths must be
/// absolute or start with `~`; anything else becomes a diagnostic.
fn path_target(
    scan: &mut Scan,
    raw: &str,
    file: &TargetPath,
    pointer: &str,
) -> Option<FindingTarget> {
    path_target_in(scan, raw, None, file, pointer)
}

/// Like [`path_target`], but relative paths are resolved against `relative_to`.
fn path_target_in(
    scan: &mut Scan,
    raw: &str,
    relative_to: Option<&TargetPath>,
    file: &TargetPath,
    pointer: &str,
) -> Option<FindingTarget> {
    let parts = split_pattern(raw);
    let base = if parts.base.is_empty() {
        raw
    } else {
        &parts.base
    };
    let resolved = resolve_path(base, &scan.locations.home, relative_to).ok();
    match resolved {
        Some(path) => Some(match parts.kind {
            PatternKind::Glob => FindingTarget::path(raw, PatternKind::Glob, path.to_path_buf()),
            _ => FindingTarget::path(raw, PatternKind::Directory, path.to_path_buf()),
        }),
        None => {
            scan.diagnostic(
                Diagnostic::new(
                    DiagnosticLevel::Warning,
                    DiagnosticCode::UnparsableRule,
                    Message::new("diagnostic.codex.relativePath").arg("path", raw),
                )
                .file(file.to_path_buf())
                .pointer(pointer),
            );
            None
        }
    }
}

fn pattern_target(raw: &str, path: &str) -> FindingTarget {
    let parts = split_pattern(path);
    let kind = match parts.kind {
        PatternKind::Glob => PatternKind::Glob,
        _ => PatternKind::Directory,
    };
    let base = if parts.base.is_empty() {
        path
    } else {
        &parts.base
    };
    FindingTarget::path(raw, kind, std::path::PathBuf::from(base))
}
