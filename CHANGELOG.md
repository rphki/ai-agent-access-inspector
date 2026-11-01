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
