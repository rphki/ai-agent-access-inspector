//! Exporting scan reports.

pub mod anonymize;
pub mod markdown;

use crate::model::ScanReport;

/// The report as pretty-printed JSON, with a final line break.
pub fn json(report: &ScanReport) -> String {
    serde_json::to_string_pretty(report).expect("a scan report always serialises") + "\n"
}
