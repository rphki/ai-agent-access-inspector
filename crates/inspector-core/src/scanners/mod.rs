//! Scanners for individual agents.

pub mod claude_code;
pub mod demo;

use crate::scanner::AgentScanner;

/// Scanners for real agents; they run by default.
pub(crate) const REAL: &[&dyn AgentScanner] = &[&claude_code::ClaudeCodeScanner];

/// Scanners that only run when selected explicitly.
pub(crate) const EXTRA: &[&dyn AgentScanner] = &[&demo::DemoScanner];
