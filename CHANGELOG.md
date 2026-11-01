# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Cargo workspace with `inspector-core` and `inspector-cli`
- CLI skeleton with `--version` and `--help`
- Data model for findings, diagnostics and scan reports with JSON serialization
- Scanner interface, scan context with system and fixture mode
- Demo scanner with fixed example findings
- Path analysis: target paths for Unix, Windows drives and UNC shares, `~` and `${VAR}` resolution, pattern base paths, existence and symlink checks, warnings for broad and sensitive grants, deduplication, containment and deny-rule conflicts
- CLI commands `scan` (JSON output, `--agent`, `--home`, `--cwd`, `--fixture`) and `list-scanners`
- CI with format check, Clippy and tests on macOS, Windows and Linux
- Message keys with an English catalog for all user-facing texts of the core, as groundwork for a translated GUI
- Dependency audit with `cargo deny` in CI, weekly CI run, Dependabot for Cargo and GitHub Actions
- `SECURITY.md` with the reporting process
- Claude Code scanner: managed settings (including drop-ins), user, project and local settings; permission rules with all four path anchors; additional directories; permission modes; sandbox; MCP servers on all levels with approvals and allow/deny policies; known projects from `.claude.json` and session logs; summary per agent
- Safe file reading (regular files only, size limits) and redaction of secret-looking values
- Markdown output (`--format markdown`, `--verbose`)
- Fixtures for macOS, Linux, Windows and malformed input with expected results
- Release workflow for CLI binaries (macOS, Linux, Windows) with checksums
- Onboarding guide for the build setup, tests and CI (`docs/onboarding.md`)
