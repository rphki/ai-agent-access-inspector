# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Export of reports as JSON or Markdown from the GUI (save dialog) with path privacy: full paths, home directory as `~`, or anonymised user names; Markdown in the GUI language
- CLI option `scan --paths full|home|anonymized`
- Redaction of secrets in all output: permission rules, values from configuration files, message parameters, HTTP authentication headers
- `docs/privacy.md`

- Desktop application (Tauri 2, React, TypeScript): main view with detected agents, last scan, counts and configuration summary; results list with filters (agent, status, effect, access, scope, warnings only, existing paths only); detail view with path, rule, source and position, explanation, confidence, path state, warnings and related findings; actions to show a folder, open a configuration file, copy the path and copy the finding as JSON
- German and English GUI, following the system language and switchable; the English texts are generated from the core catalog
- Tauri commands `list_scanners`, `run_scan`, `get_scan_details`, `get_app_info`, `open_path_in_file_manager`, `open_source_file`; paths are only opened if the last scan reported them, and only JSON and Markdown files are handed to their default application
- Frontend checks in CI (type check, ESLint, Vitest, build, `pnpm audit`); Dependabot for npm

### Changed

- Minimum Rust version raised to 1.88, so that dependency updates with security fixes can be used

## [0.1.0] - 2026-11-01

First release: a command line tool that shows which folders Claude Code can access according to its configuration. Pre-release; builds are not signed yet.

### Added

- Claude Code scanner: managed settings (including drop-ins), user, project and local settings; permission rules with all four path anchors; additional directories; permission modes; sandbox; MCP servers on all levels with approvals and allow/deny policies; known projects from `.claude.json` and session logs; summary per agent
- Path analysis: target paths for Unix, Windows drives and UNC shares, `~` and `${VAR}` resolution, pattern base paths, existence and symlink checks, warnings for broad and sensitive grants, deduplication, containment and deny-rule conflicts
- CLI `ai-agent-access-inspector` with `scan` (JSON and Markdown output, `--agent`, `--home`, `--cwd`, `--fixture`, `--verbose`) and `list-scanners`
- Data model for findings, diagnostics and scan reports with JSON serialization; message keys with an English catalog as groundwork for a translated GUI
- Safe file reading (regular files only, size limits) and redaction of secret-looking values
- Demo scanner with fixed example findings
- Fixtures for macOS, Linux, Windows and malformed input with expected results
- CI on macOS, Windows and Linux with format check, Clippy, tests and `cargo deny`; weekly run; Dependabot
- Release workflow for CLI archives (macOS, Linux, Windows) with checksums
- Documentation: roadmap, architecture, data model, Claude Code scanner specification, onboarding, release process, security checks, `SECURITY.md`

[Unreleased]: https://github.com/rphki/ai-agent-access-inspector/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/rphki/ai-agent-access-inspector/releases/tag/v0.1.0
