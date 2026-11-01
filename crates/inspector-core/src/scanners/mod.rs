//! Scanners for individual agents.

pub mod demo;

use crate::scanner::AgentScanner;

/// Scanners for real agents; they run by default.
pub(crate) const REAL: &[&dyn AgentScanner] = &[];

/// Scanners that only run when selected explicitly.
pub(crate) const EXTRA: &[&dyn AgentScanner] = &[&demo::DemoScanner];
