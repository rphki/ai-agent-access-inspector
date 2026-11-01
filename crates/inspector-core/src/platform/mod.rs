//! Operating system specifics.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperatingSystem {
    #[serde(rename = "macos")]
    MacOs,
    Linux,
    Windows,
}

impl OperatingSystem {
    /// The operating system this binary was built for.
    pub fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(windows) {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_as_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&OperatingSystem::MacOs).unwrap(),
            "\"macos\""
        );
        assert_eq!(
            serde_json::to_string(&OperatingSystem::Windows).unwrap(),
            "\"windows\""
        );
    }
}
