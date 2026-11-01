# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Language

- **English** for everything written into the repository: documentation, code, comments, CLI output, commit messages, PR descriptions, issues.
- Conversation with the maintainer may be in German.

## Project

AI Agent Access Inspector: a local, cross-platform tool that shows which folders local AI agents (first: Claude Code) can access according to their configuration. Read-only, no network, no telemetry.

- Planning and status: `docs/roadmap.md` (authoritative; `docs/plan-initial.md` is historical only)
- Claude Code scanner specification: `docs/claude-code-scanner.md`
- Data model and JSON format: `docs/findings-model.md`
- Architecture: `docs/architecture.md`

## Layout

| Path | Content |
|---|---|
| `crates/inspector-core` | data model, scan context, scanners, path analysis; no Tauri dependency |
| `crates/inspector-cli` | CLI binary `ai-agent-access-inspector` |
| `src-tauri/`, `src/` | GUI, from phase 5 |
| `fixtures/` | test fixtures with invented paths and values only |

## Commands

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p inspector-cli -- scan --agent demo
```

All three checks must pass before a commit. CI runs them on macOS, Windows and Linux.

## Rules

- **Never modify agent configuration.** The tool only reads.
- **Scanners access the system only through `ScanContext`**, never `std::env` or the real home directory directly. System-wide paths go through `context.system_path(...)`.
- **Scanners never fail as a whole.** Problems become `Diagnostic`s.
- **Privacy:** read only the keys a scanner needs; never output secrets (MCP `env`, `headers`, tokens). When inspecting the maintainer's local configuration for research, look at structure and key names only, not values.
- **Undocumented sources** are marked `heuristic` and get at most medium confidence.
- **Dependencies in `inspector-core`** are added sparingly.
- Keep `docs/roadmap.md` (status table) and `CHANGELOG.md` (`Unreleased`) up to date when a phase or feature is done.

## Workflow

- Work proceeds phase by phase as listed in `docs/roadmap.md`; each phase ends with its acceptance check.
- Commit and push only when the maintainer asks.
- Commit messages: imperative English subject line (≤ 72 characters), blank line, bullet list of changes.
- Commits use the repository-local GitHub noreply address; do not change the git identity.
