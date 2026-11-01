# Changelog

All notable changes are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), versioning follows [Semantic Versioning](https://semver.org/).

## [Unreleased]

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
