# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- New application icon: magnifying glass over a folder labelled "AI"
- GUI colour scheme: orange as the highlight colour, blue only for focus, info banners and low severity; neutral greys
- macOS About panel: copyright "Raphael Kirchner" on two lines, version shown once

## [0.4.0] - 2026-11-01

First release of the desktop application, for macOS, Windows and Linux. Pre-release; builds are not signed yet (see the README for how to open them). Versions 0.2.0 (GUI) and 0.3.0 (export) were not released separately; their content is part of this release.

### Added

- Desktop application (Tauri 2, React, TypeScript): main view with detected agents, last scan, counts and configuration summary; results list with filters (agent, status, effect, access, scope, warnings only, existing paths only); detail view with path, rule, source and position, explanation, confidence, path state, warnings and related findings; actions to show a folder, open a configuration file, copy the path and copy the finding as JSON
- German and English GUI, following the system language and switchable; the English texts are generated from the core catalog
- Export of reports as JSON or Markdown from the GUI (save dialog) with path privacy: full paths, home directory as `~`, or anonymised user names; Markdown in the GUI language
- CLI option `scan --paths full|home|anonymized`
- Redaction of secrets in all output: permission rules, values from configuration files, message parameters, HTTP authentication headers
- Release builds of the GUI: macOS universal `.dmg`, Windows `-setup.exe` and `.msi`, Linux `.AppImage` and `.deb`, with checksums
- Documentation: privacy, threat model, contributing guide, guide for adding a scanner, issue templates; README with screenshots and installation notes
- Frontend checks in CI (type check, ESLint, Vitest, build, `pnpm audit`); Dependabot for npm

### Changed

- Minimum Rust version raised to 1.88, so that dependency updates with security fixes can be used
- GitHub Actions are pinned to commit SHAs; release builds use no caches

### Security

- Paths are only opened if the last scan reported them, and only JSON and Markdown files are handed to their default application; exports are written only to the file chosen in the save dialog

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

[Unreleased]: https://github.com/rphki/ai-agent-access-inspector/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/rphki/ai-agent-access-inspector/compare/v0.1.0...v0.4.0
[0.1.0]: https://github.com/rphki/ai-agent-access-inspector/releases/tag/v0.1.0
