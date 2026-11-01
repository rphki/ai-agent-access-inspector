use std::path::PathBuf;

use super::*;
use crate::context::EnvSnapshot;
use crate::model::{
    FindingScope, FindingSource, FindingStatus, FindingTarget, PathKind, ScannerMetadata,
    SourceDocumentation,
};
use crate::platform::OperatingSystem::{self, Linux, MacOs, Windows};
use crate::test_support::TempDir;

struct Fixture {
    /// Keeps the temporary directory alive for the test's duration.
    _dir: TempDir,
    context: ScanContext,
}

impl Fixture {
    fn new(os: OperatingSystem, home: &str) -> Self {
        let dir = TempDir::new("analysis");
        let context = ScanContext {
            home_dir: PathBuf::from(home),
            current_dir: None,
            operating_system: os,
            env: EnvSnapshot::default(),
            system_root: Some(dir.path().join("system")),
        };
        Self { _dir: dir, context }
    }

    fn mkdir(&self, target: &str) {
        std::fs::create_dir_all(self.context.host_path(target)).unwrap();
    }

    fn analyze(&self, findings: Vec<Finding>) -> Vec<Finding> {
        analyze(findings, &self.context)
    }
}

fn agent() -> ScannerMetadata {
    ScannerMetadata {
        id: "test".into(),
        name: "Test".into(),
        description: String::new(),
        parser_version: "1".into(),
    }
}

fn finding(base: &str, kind: PatternKind, effect: RuleEffect, access: AccessKind) -> Finding {
    finding_from(base, kind, effect, access, "/settings.json")
}

fn finding_from(
    base: &str,
    kind: PatternKind,
    effect: RuleEffect,
    access: AccessKind,
    file: &str,
) -> Finding {
    Finding::builder(
        &agent(),
        FindingScope::User,
        FindingTarget::path(base, kind, PathBuf::from(base)),
        FindingSource::new("test/settings", SourceDocumentation::Documented, "1")
            .file(file)
            .pointer("/x"),
    )
    .effect(effect)
    .access(access)
    .status(FindingStatus::Configured)
    .build()
}

fn allow_dir(base: &str) -> Finding {
    finding(
        base,
        PatternKind::Directory,
        RuleEffect::Allow,
        AccessKind::ReadWrite,
    )
}

fn deny_dir(base: &str) -> Finding {
    finding(
        base,
        PatternKind::Directory,
        RuleEffect::Deny,
        AccessKind::Read,
    )
}

fn warning_kinds(finding: &Finding) -> Vec<WarningKind> {
    finding.warnings.iter().map(|w| w.kind.clone()).collect()
}

#[test]
fn normalises_base_and_display_paths() {
    let fx = Fixture::new(Linux, "/home/alice");
    let result = fx.analyze(vec![allow_dir("/home/alice/x/../projects/")]);
    assert_eq!(
        result[0].target.base_path,
        Some(PathBuf::from("/home/alice/projects"))
    );
    assert_eq!(result[0].target.display_path, "/home/alice/projects");

    let fx = Fixture::new(Windows, r"C:\Users\alice");
    let result = fx.analyze(vec![allow_dir("c:/Users/alice/Projects")]);
    assert_eq!(result[0].target.display_path, r"C:\Users\alice\Projects");
}

#[test]
fn checks_existence_through_the_fixture() {
    let fx = Fixture::new(Windows, r"C:\Users\alice");
    fx.mkdir(r"C:\Users\alice\projects");
    let result = fx.analyze(vec![
        allow_dir(r"C:\Users\alice\projects"),
        allow_dir(r"C:\Users\alice\gone"),
        deny_dir(r"C:\Users\alice\also-gone"),
    ]);
    assert_eq!(result[0].path_state.existence, Existence::Exists);
    assert_eq!(result[0].path_state.kind, PathKind::Directory);
    assert!(result[0].warnings.is_empty());
    assert_eq!(result[1].path_state.existence, Existence::Missing);
    assert_eq!(warning_kinds(&result[1]), [WarningKind::MissingPath]);
    // A deny rule for a missing path is harmless.
    assert!(result[2].warnings.is_empty());
}

#[test]
fn findings_without_a_path_are_left_alone() {
    let fx = Fixture::new(Linux, "/home/alice");
    let shell = Finding::builder(
        &agent(),
        FindingScope::OperatingSystem,
        FindingTarget::unresolved("Bash", PatternKind::Everything),
        FindingSource::new("test/derived", SourceDocumentation::Derived, "1"),
    )
    .effect(RuleEffect::Allow)
    .build();
    let relative = finding(
        "src",
        PatternKind::Directory,
        RuleEffect::Allow,
        AccessKind::Read,
    );
    let result = fx.analyze(vec![shell.clone(), relative.clone()]);
    assert_eq!(result, vec![shell, relative]);
}

#[test]
fn warns_about_broad_grants() {
    let fx = Fixture::new(Linux, "/home/alice");
    let result = fx.analyze(vec![
        allow_dir("/"),
        allow_dir("/home"),
        allow_dir("/home/alice"),
        allow_dir("/home/alice/.ssh/keys"),
        allow_dir("/home/alice/Documents"),
        allow_dir("/home/alice/Documents/project"),
    ]);
    let kinds: Vec<Vec<WarningKind>> = result
        .iter()
        .map(|f| {
            warning_kinds(f)
                .into_iter()
                .filter(|k| *k != WarningKind::MissingPath)
                .collect()
        })
        .collect();
    assert_eq!(
        kinds,
        vec![
            vec![WarningKind::FilesystemRoot],
            vec![WarningKind::BroadPath],
            vec![WarningKind::HomeDirectory],
            vec![WarningKind::SensitiveLocation],
            vec![WarningKind::BroadPath],
            vec![],
        ]
    );
    assert_eq!(result[1].warnings[0].severity, Severity::High);
    assert_eq!(result[4].warnings[0].severity, Severity::Medium);
}

#[test]
fn warns_about_whole_drives_and_shares() {
    let fx = Fixture::new(Windows, r"C:\Users\alice");
    let result = fx.analyze(vec![allow_dir(r"D:\"), allow_dir(r"\\nas\team")]);
    for finding in &result {
        assert!(warning_kinds(finding).contains(&WarningKind::WholeDrive));
    }
}

#[test]
fn deny_rules_get_no_breadth_warnings() {
    let fx = Fixture::new(Linux, "/home/alice");
    fx.mkdir("/home/alice");
    let result = fx.analyze(vec![deny_dir("/home/alice")]);
    assert!(result[0].warnings.is_empty());
}

#[test]
fn home_check_respects_letter_case_rules() {
    let mac = Fixture::new(MacOs, "/Users/alice");
    let result = mac.analyze(vec![allow_dir("/users/ALICE")]);
    assert!(warning_kinds(&result[0]).contains(&WarningKind::HomeDirectory));

    let linux = Fixture::new(Linux, "/home/alice");
    let result = linux.analyze(vec![allow_dir("/home/ALICE")]);
    assert!(!warning_kinds(&result[0]).contains(&WarningKind::HomeDirectory));
}

#[test]
fn merges_findings_for_the_same_location() {
    let fx = Fixture::new(MacOs, "/Users/alice");
    let user = finding_from(
        "/Users/alice/projects",
        PatternKind::Directory,
        RuleEffect::Allow,
        AccessKind::ReadWrite,
        "/Users/alice/.claude/settings.json",
    );
    let project = finding_from(
        "/users/alice/x/../Projects/",
        PatternKind::Directory,
        RuleEffect::Allow,
        AccessKind::ReadWrite,
        "/Users/alice/app/.claude/settings.json",
    );
    let denied = deny_dir("/Users/alice/projects");
    let result = fx.analyze(vec![user.clone(), project.clone(), denied]);

    assert_eq!(result.len(), 2);
    assert_eq!(result[0].id, user.id);
    assert_eq!(result[0].additional_sources, vec![project.source]);
}

#[test]
fn does_not_merge_on_case_sensitive_systems() {
    let fx = Fixture::new(Linux, "/home/alice");
    let result = fx.analyze(vec![allow_dir("/srv/Data"), allow_dir("/srv/data")]);
    assert_eq!(result.len(), 2);
}

#[test]
fn globs_are_only_merged_when_written_identically() {
    let fx = Fixture::new(Linux, "/home/alice");
    let glob = |raw: &str| {
        let mut f = finding(
            "/srv",
            PatternKind::Glob,
            RuleEffect::Allow,
            AccessKind::Read,
        );
        f.target.raw = raw.into();
        f
    };
    let result = fx.analyze(vec![
        glob("/srv/*.md"),
        glob("/srv/*.md"),
        glob("/srv/*.txt"),
    ]);
    assert_eq!(result.len(), 2);
}

#[test]
fn records_containment() {
    let fx = Fixture::new(Linux, "/home/alice");
    let parent = allow_dir("/srv");
    let child = allow_dir("/srv/data");
    let result = fx.analyze(vec![parent.clone(), child.clone()]);
    assert_eq!(
        result[0].related,
        vec![RelatedFinding {
            relation: Relation::Contains,
            finding_id: child.id
        }]
    );
    assert_eq!(
        result[1].related,
        vec![RelatedFinding {
            relation: Relation::ContainedIn,
            finding_id: parent.id
        }]
    );
}

#[test]
fn deny_rules_override_covered_allow_rules() {
    let fx = Fixture::new(Linux, "/home/alice");
    let deny = deny_dir("/home/alice/.ssh");
    let allow_same = allow_dir("/home/alice/.ssh");
    let allow_below = finding(
        "/home/alice/.ssh/config",
        PatternKind::ExactPath,
        RuleEffect::Ask,
        AccessKind::Read,
    );
    let result = fx.analyze(vec![deny.clone(), allow_same, allow_below]);

    for overridden in &result[1..] {
        assert!(overridden.related.contains(&RelatedFinding {
            relation: Relation::OverriddenBy,
            finding_id: deny.id.clone(),
        }));
        assert!(warning_kinds(overridden).contains(&WarningKind::ConflictingRules));
    }
    // The deny rule itself carries no conflict warning.
    assert!(!warning_kinds(&result[0]).contains(&WarningKind::ConflictingRules));
}

#[test]
fn partial_denies_do_not_override() {
    let fx = Fixture::new(Linux, "/home/alice");
    let glob_deny = finding(
        "/srv",
        PatternKind::Glob,
        RuleEffect::Deny,
        AccessKind::Read,
    );
    let write_deny = finding(
        "/data",
        PatternKind::Directory,
        RuleEffect::Deny,
        AccessKind::Write,
    );
    let result = fx.analyze(vec![
        glob_deny,
        allow_dir("/srv/app"),
        write_deny,
        finding(
            "/data/x",
            PatternKind::Directory,
            RuleEffect::Allow,
            AccessKind::Read,
        ),
    ]);
    for finding in &result {
        assert!(!warning_kinds(finding).contains(&WarningKind::ConflictingRules));
    }
}

#[cfg(unix)]
#[test]
fn warns_about_symlinks_pointing_elsewhere() {
    use std::os::unix::fs::symlink;

    let fx = Fixture::new(Linux, "/home/alice");
    fx.mkdir("/srv/secret");
    fx.mkdir("/home/alice/projects/real");
    symlink(
        fx.context.host_path("/srv/secret"),
        fx.context.host_path("/home/alice/projects/link"),
    )
    .unwrap();
    symlink(
        fx.context.host_path("/home/alice/projects/real"),
        fx.context.host_path("/home/alice/projects/local"),
    )
    .unwrap();

    let result = fx.analyze(vec![
        allow_dir("/home/alice/projects/link"),
        allow_dir("/home/alice/projects/local"),
    ]);
    assert_eq!(
        result[0].path_state.symlink_target,
        Some(PathBuf::from("/srv/secret"))
    );
    assert!(warning_kinds(&result[0]).contains(&WarningKind::SymlinkElsewhere));
    assert_eq!(
        result[1].path_state.symlink_target,
        Some(PathBuf::from("/home/alice/projects/real"))
    );
    assert!(result[1].warnings.is_empty());
}
