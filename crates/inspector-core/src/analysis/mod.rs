//! Path analysis shared by all scanners.
//!
//! Scanners use [`resolve`] and [`pattern`] to turn configured values into
//! target paths. After a scanner has run, [`analyze`] normalises the base
//! paths of its findings, checks them on disk, adds warnings, merges
//! duplicates and records how findings relate to each other.

pub mod fs;
pub mod path;
pub mod pattern;
pub mod resolve;

use std::collections::HashMap;

use crate::context::ScanContext;
use crate::model::{
    AccessKind, Existence, Finding, FindingSubject, PathState, PatternKind, RelatedFinding,
    Relation, RuleEffect, Severity, Warning, WarningKind,
};
use path::TargetPath;

/// Folders below the home directory that typically hold credentials.
const SENSITIVE_LOCATIONS: &[&str] = &[
    ".ssh",
    ".aws",
    ".azure",
    ".gnupg",
    ".kube",
    ".docker",
    ".config/gcloud",
    "Library/Keychains",
];

/// Folders below the home directory that hold personal files.
const PERSONAL_FOLDERS: &[&str] = &[
    "Desktop",
    "Documents",
    "Downloads",
    "Library",
    "Movies",
    "Music",
    "OneDrive",
    "Pictures",
    "Videos",
    "AppData",
];

/// Runs the path analysis over the findings of one scanner.
pub fn analyze(findings: Vec<Finding>, context: &ScanContext) -> Vec<Finding> {
    let home = context.home();
    let mut analyzed: Vec<(Finding, Option<TargetPath>)> = findings
        .into_iter()
        .map(|mut finding| {
            let path = normalize_target(&mut finding, context);
            if let Some(path) = &path {
                check_on_disk(&mut finding, path, context);
                add_breadth_warnings(&mut finding, path, home.as_ref());
            }
            (finding, path)
        })
        .collect();
    analyzed = merge_duplicates(analyzed);
    relate(&mut analyzed);
    analyzed
        .into_iter()
        .map(|(mut finding, _)| {
            // Most severe first; stable, so equal severities keep their order.
            finding
                .warnings
                .sort_by_key(|w| std::cmp::Reverse(w.severity));
            finding
        })
        .collect()
}

/// Parses and normalises the finding's base path. Returns `None` for
/// findings without a usable absolute path.
fn normalize_target(finding: &mut Finding, context: &ScanContext) -> Option<TargetPath> {
    if finding.target.pattern_kind == PatternKind::Everything {
        return None;
    }
    let base = finding.target.base_path.as_ref()?;
    let path = TargetPath::parse(context.operating_system, &base.to_string_lossy()).ok()?;
    if !path.is_absolute() {
        return None;
    }
    finding.target.base_path = Some(path.to_path_buf());
    finding.target.display_path = path.to_string();
    Some(path)
}

fn check_on_disk(finding: &mut Finding, path: &TargetPath, context: &ScanContext) {
    let state = fs::inspect(&context.host_path(&path.to_string()));
    let symlink_target = state
        .symlink_target
        .as_ref()
        .and_then(|host| context.target_path(host))
        .and_then(|target| TargetPath::parse(context.operating_system, &target).ok());
    finding.path_state = PathState {
        existence: state.existence,
        kind: state.kind,
        symlink_target: symlink_target.as_ref().map(TargetPath::to_path_buf),
    };

    if state.existence == Existence::Missing && finding.effect != RuleEffect::Deny {
        finding.warnings.push(Warning::new(
            WarningKind::MissingPath,
            Severity::Low,
            "The path does not exist (anymore).",
        ));
    }
    if let (Some(target), Some(parent)) = (&symlink_target, path.parent()) {
        if !target.starts_with(&parent) {
            finding.warnings.push(Warning::new(
                WarningKind::SymlinkElsewhere,
                Severity::Medium,
                format!("Symlink to {target}, outside of {parent}."),
            ));
        }
    }
}

fn add_breadth_warnings(finding: &mut Finding, path: &TargetPath, home: Option<&TargetPath>) {
    if !matches!(finding.effect, RuleEffect::Allow | RuleEffect::Ask) {
        return;
    }
    let mut warn = |kind, severity, message: String| {
        finding.warnings.push(Warning::new(kind, severity, message));
    };

    if path.is_drive_root() {
        warn(
            WarningKind::WholeDrive,
            Severity::High,
            format!("Grants the whole drive {path}."),
        );
        return;
    }
    if path.is_root() {
        warn(
            WarningKind::FilesystemRoot,
            Severity::High,
            "Grants the entire filesystem.".into(),
        );
        return;
    }
    let Some(home) = home else {
        return;
    };
    if path.same_path(home) {
        warn(
            WarningKind::HomeDirectory,
            Severity::High,
            "Grants the entire home directory with all personal files.".into(),
        );
    } else if home.starts_with(path) {
        warn(
            WarningKind::BroadPath,
            Severity::High,
            format!("Grants {path}, which contains the home directory {home}."),
        );
    } else if let Some(location) = matching_location(path, home, SENSITIVE_LOCATIONS) {
        warn(
            WarningKind::SensitiveLocation,
            Severity::High,
            format!("{location} typically holds credentials."),
        );
    } else if let Some(folder) = PERSONAL_FOLDERS
        .iter()
        .filter_map(|f| home.join(f).ok())
        .find(|folder| path.same_path(folder))
    {
        warn(
            WarningKind::BroadPath,
            Severity::Medium,
            format!("Grants the personal folder {folder}."),
        );
    }
}

/// The first location (relative to `home`) that `path` lies in.
fn matching_location(
    path: &TargetPath,
    home: &TargetPath,
    locations: &[&str],
) -> Option<TargetPath> {
    locations
        .iter()
        .filter_map(|location| home.join(location).ok())
        .find(|location| path.starts_with(location))
}

/// Findings that say the same thing about the same path are merged into the
/// first one; the others' sources go to `additional_sources`.
fn merge_duplicates(
    findings: Vec<(Finding, Option<TargetPath>)>,
) -> Vec<(Finding, Option<TargetPath>)> {
    let mut merged: Vec<(Finding, Option<TargetPath>)> = Vec::new();
    let mut index: HashMap<String, usize> = HashMap::new();
    for (finding, path) in findings {
        let key = duplicate_key(&finding, path.as_ref());
        match index.get(&key) {
            Some(&i) => {
                let primary = &mut merged[i].0;
                primary.additional_sources.push(finding.source);
                primary
                    .additional_sources
                    .extend(finding.additional_sources);
                for warning in finding.warnings {
                    if !primary.warnings.contains(&warning) {
                        primary.warnings.push(warning);
                    }
                }
            }
            None => {
                index.insert(key, merged.len());
                merged.push((finding, path));
            }
        }
    }
    merged
}

fn duplicate_key(finding: &Finding, path: Option<&TargetPath>) -> String {
    let subject = match &finding.subject {
        FindingSubject::Agent => String::new(),
        FindingSubject::McpServer { name } => name.clone(),
    };
    // Globs are only equal if written the same way; paths if they resolve
    // to the same location, however they were written.
    let target = match (path, finding.target.pattern_kind) {
        (Some(path), kind) if kind != PatternKind::Glob => path.comparison_key(),
        _ => finding.target.raw.clone(),
    };
    format!(
        "{}\0{subject}\0{:?}\0{:?}\0{:?}\0{:?}\0{target}",
        finding.agent_id,
        finding.effect,
        finding.access,
        finding.status,
        finding.target.pattern_kind
    )
}

/// Records containment between findings and marks allow/ask findings that a
/// deny rule overrides.
fn relate(findings: &mut [(Finding, Option<TargetPath>)]) {
    let mut relations: Vec<(usize, Relation, usize)> = Vec::new();
    for (i, (a, a_path)) in findings.iter().enumerate() {
        let Some(a_path) = a_path else { continue };
        for (j, (b, b_path)) in findings.iter().enumerate() {
            let Some(b_path) = b_path else { continue };
            if i == j || a.agent_id != b.agent_id || a.subject != b.subject {
                continue;
            }
            if overrides(b, b_path, a, a_path) {
                relations.push((i, Relation::OverriddenBy, j));
            } else if a_path.starts_with(b_path) && !a_path.same_path(b_path) {
                relations.push((i, Relation::ContainedIn, j));
                relations.push((j, Relation::Contains, i));
            }
        }
    }
    for (i, relation, j) in relations {
        let other = findings[j].0.clone();
        let finding = &mut findings[i].0;
        finding.related.push(RelatedFinding {
            relation,
            finding_id: other.id.clone(),
        });
        if relation == Relation::OverriddenBy {
            finding.warnings.push(Warning::new(
                WarningKind::ConflictingRules,
                Severity::Medium,
                format!(
                    "Overridden by deny rule {}; deny takes precedence.",
                    other.target.raw
                ),
            ));
        }
    }
}

/// Whether deny finding `deny` covers allow/ask finding `other` entirely.
fn overrides(
    deny: &Finding,
    deny_path: &TargetPath,
    other: &Finding,
    other_path: &TargetPath,
) -> bool {
    deny.effect == RuleEffect::Deny
        && matches!(other.effect, RuleEffect::Allow | RuleEffect::Ask)
        // A glob deny only covers some files below its base path.
        && deny.target.pattern_kind != PatternKind::Glob
        && accesses_overlap(deny.access, other.access)
        && other_path.starts_with(deny_path)
}

fn accesses_overlap(a: AccessKind, b: AccessKind) -> bool {
    use AccessKind::*;
    match (a, b) {
        (Unknown, _) | (_, Unknown) => true,
        (ReadWrite, _) | (_, ReadWrite) => true,
        (a, b) => a == b,
    }
}

#[cfg(test)]
mod tests;
