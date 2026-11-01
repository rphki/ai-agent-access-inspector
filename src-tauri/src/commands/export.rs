//! Rendering of exports; writing happens in `export_scan`.

use std::collections::HashMap;

use inspector_core::export::anonymize::{PathPrivacy, anonymize};
use inspector_core::export::markdown::{self, MarkdownOptions};
use serde::Deserialize;

use super::StoredScan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    Json,
    Markdown,
}

impl ExportFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Markdown => "md",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Json => "JSON",
            Self::Markdown => "Markdown",
        }
    }
}

pub fn render(
    stored: &StoredScan,
    format: ExportFormat,
    privacy: PathPrivacy,
    translations: Option<&HashMap<String, String>>,
) -> String {
    let report = anonymize(&stored.report, &stored.home, privacy);
    match format {
        ExportFormat::Json => inspector_core::export::json(&report),
        ExportFormat::Markdown => markdown::render(
            &report,
            MarkdownOptions {
                verbose: true,
                translations,
            },
        ),
    }
}

/// `ai-agent-access-report-2026-11-01.md`, dated by the end of the scan (UTC).
pub fn file_name(finished_at_ms: u64, format: ExportFormat) -> String {
    let (year, month, day) = civil_date(finished_at_ms / 86_400_000);
    format!(
        "ai-agent-access-report-{year:04}-{month:02}-{day:02}.{}",
        format.extension()
    )
}

/// Gregorian date of a day count since 1970-01-01 (Howard Hinnant's
/// `civil_from_days`).
fn civil_date(days: u64) -> (u64, u64, u64) {
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + u64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use inspector_core::model::{ScanReport, ScanResult, ToolInfo};
    use inspector_core::platform::OperatingSystem;

    fn stored() -> StoredScan {
        let json = include_str!("../../../fixtures/claude-code/secrets/expected.json");
        let result: ScanResult = serde_json::from_str(json).unwrap();
        StoredScan {
            report: ScanReport {
                format_version: "test".into(),
                tool: ToolInfo::current(),
                operating_system: OperatingSystem::Linux,
                results: vec![result],
            },
            finished_at: 1_793_534_400_000,
            home: "/home/carol".into(),
        }
    }

    #[test]
    fn file_names_carry_the_scan_date() {
        assert_eq!(
            file_name(0, ExportFormat::Json),
            "ai-agent-access-report-1970-01-01.json"
        );
        assert_eq!(
            file_name(1_793_534_400_000, ExportFormat::Markdown),
            "ai-agent-access-report-2026-11-01.md"
        );
        assert_eq!(civil_date(11_016), (2000, 2, 29));
    }

    #[test]
    fn exports_apply_privacy_and_translations() {
        let stored = stored();
        let json = render(&stored, ExportFormat::Json, PathPrivacy::Anonymized, None);
        assert!(!json.contains("carol"));
        assert!(json.contains("\"key\""), "JSON keeps message keys");

        let full = render(&stored, ExportFormat::Markdown, PathPrivacy::Full, None);
        assert!(full.contains("/home/carol"));
        assert!(!full.contains("fixture-"), "secrets are redacted");

        let german = HashMap::from([(
            "finding.mcpServer".to_string(),
            "Lokaler MCP-Server '{server}'.".to_string(),
        )]);
        let translated = render(
            &stored,
            ExportFormat::Markdown,
            PathPrivacy::Home,
            Some(&german),
        );
        assert!(translated.contains("Lokaler MCP-Server 'wrapped'."));
        assert!(!translated.contains("/home/carol"));
    }
}
