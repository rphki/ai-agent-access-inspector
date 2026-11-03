# CLAUDE.md

Guidance for Claude Code when working in this repository.

## Language

- **English** for everything written into the repository: documentation, code, comments, CLI output, commit messages, PR descriptions, issues.
- Conversation with the maintainer may be in German.
- **Internationalisation:** only the GUI is translated (German, English); the CLI is English only. User-facing texts in `inspector-core` are never plain strings but `Message`s with a key from the English catalog (`model/message.rs`); see `docs/architecture.md`, section 6.

## Project

AI Agent Access Inspector: a local, cross-platform tool that shows which folders local AI agents (first: Claude Code) can access according to their configuration. Read-only, no network, no telemetry.

- Planning and status: `docs/roadmap.md` (authoritative; `docs/plan-initial.md` is historical only)
- Claude Code scanner specification: `docs/claude-code-scanner.md`
- ChatGPT/Codex scanner specification: `docs/codex-scanner.md`
- Data model and JSON format: `docs/findings-model.md`
- Architecture: `docs/architecture.md`
- Build setup, tests and CI: `docs/onboarding.md` (keep it current when the build or CI changes)
- Privacy (what is read, redaction, export anonymisation): `docs/privacy.md`

## Layout

| Path | Content |
|---|---|
| `crates/inspector-core` | data model, scan context, scanners, path analysis; no Tauri dependency |
| `crates/inspector-cli` | CLI binary `ai-agent-access-inspector` |
| `src-tauri/` | GUI crate `inspector-gui`: Tauri commands and configuration |
| `src/` | GUI frontend (React, TypeScript); `src/locales/en.json` is generated from the core catalog |
| `fixtures/` | test fixtures with invented paths and values only |

## Commands

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p inspector-cli -- scan --agent demo
pnpm typecheck && pnpm lint && pnpm test
pnpm tauri dev
```

The Cargo checks and the frontend checks must pass before a commit. CI runs them on macOS, Windows and Linux. Platform-specific code (`#[cfg(...)]`) should also be checked with `--target x86_64-pc-windows-gnu` and `--target x86_64-unknown-linux-gnu` and `--exclude inspector-gui` (see `docs/onboarding.md`, section 5).

## Rules

- **Never modify agent configuration.** The tool only reads.
- **Scanners access the system only through `ScanContext`**, never `std::env` or the real home directory directly. Paths are target paths of the scanned system (`analysis::path::TargetPath`, never `std::path` logic, which follows the host); files are read via `context.host_path(...)`.
- **Scanners never fail as a whole.** Problems become `Diagnostic`s.
- **Privacy:** read only the keys a scanner needs; never output secrets (MCP `env`, `headers`, tokens). When inspecting the maintainer's local configuration for research, look at structure and key names only, not values.
- **Fixtures:** `fixtures/claude-code/*/expected.json` and `fixtures/codex/*/expected.json` are reviewed expectations. After an intended scanner change, regenerate with `UPDATE_EXPECT=1 cargo test -p inspector-core --test claude_code_fixtures` (or `--test codex_fixtures`) and review the diff before committing.
- **Undocumented sources** are marked `heuristic` and get at most medium confidence.
- **Dependencies in `inspector-core`** are added sparingly.
- **GUI texts:** after changing the core catalog, regenerate `src/locales/en.json` with `UPDATE_EXPECT=1 cargo test -p inspector-core gui_catalog` and add the German text to `src/locales/de.json`.
- **Security:** run `cargo deny check` (Cargo) or `pnpm audit` (npm) when changing dependencies; each phase has a security check in `docs/roadmap.md` that must be done before the phase is accepted.
- Keep `docs/roadmap.md` (status table) and `CHANGELOG.md` (`Unreleased`) up to date when a phase or feature is done.

## Working style

- **Local first.** Build and test on the maintainer's Mac. If `cargo` is not on `PATH` in the shell, run `. "$HOME/.cargo/env"` first. Install missing tools only after asking.
- **Fast inner loop:** `cargo check`, then targeted tests (`cargo test -p <crate> --test <name>` or a name filter). Run the full checks from *Commands* once at the end of a step, before a commit. Cross-target clippy only when `#[cfg]` code changed.
- **CI is the final gate**, not part of the inner loop: do not wait for CI after every push. Full cross-platform builds run in the release workflow (tag or manual).
- **No over-implementation:** do only the requested roadmap step; no unrequested features, frameworks, abstractions or infrastructure. Ask before taking on extra tasks.
- **Be economical:** don't repeat checks that already passed without a reason; fix problems in a targeted way instead of rewriting; keep command output short (`-q`, `| tail`) and don't load large files or logs into context.
- **Progress lives in files:** after each step, update status and open points in `docs/roadmap.md`, so work can continue in a fresh session.
- **Model:** Sonnet with medium effort for routine implementation; Opus or higher effort for architecture decisions and hard problems (chosen per session in the app).

## Workflow

- Work proceeds phase by phase as listed in `docs/roadmap.md`; each phase ends with its acceptance check.
- Commit and push only when the maintainer asks.
- Commit messages: imperative English subject line (≤ 72 characters), blank line, bullet list of changes.
- Commits use the repository-local GitHub noreply address; do not change the git identity.
