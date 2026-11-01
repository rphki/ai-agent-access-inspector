//! Core of the AI Agent Access Inspector.
//!
//! Contains the agent-independent data model, the path analysis and the
//! scanners for individual agents. This crate has no dependency on Tauri so
//! that it can be used and tested from the CLI and on every platform.

pub mod context;
pub mod model;
pub mod platform;
pub mod scanner;
pub mod scanners;

/// Version of the inspector core, taken from the workspace manifest.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Name of the application as shown to users and in exports.
pub const APP_NAME: &str = "ai-agent-access-inspector";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert_eq!(parts.len(), 3, "expected MAJOR.MINOR.PATCH, got {VERSION}");
        for part in parts {
            let numeric = part.split('-').next().unwrap();
            assert!(
                numeric.parse::<u64>().is_ok(),
                "invalid component in {VERSION}"
            );
        }
    }
}
