# Contributing

Thank you for your interest in the AI Agent Access Inspector. Bug reports, scanner requests and pull requests are welcome.

The tool is read-only, works offline and collects no telemetry. Every contribution has to keep it that way.

## Setup

Prerequisites, the first build, the test setup and CI are described in [docs/onboarding.md](docs/onboarding.md). Start there.

Further reading:

- [docs/architecture.md](docs/architecture.md): crates, scan flow, scan context, path analysis
- [docs/findings-model.md](docs/findings-model.md): data model and JSON format
- [docs/adding-a-scanner.md](docs/adding-a-scanner.md): step-by-step guide for a new agent scanner
- [docs/privacy.md](docs/privacy.md): what is read, redaction, export anonymisation
- [docs/roadmap.md](docs/roadmap.md): planning and status

## Before you open a pull request

All of these checks must pass. CI runs them on macOS, Windows and Linux.

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
pnpm typecheck && pnpm lint && pnpm test
```

If you changed platform-specific code (`#[cfg(...)]`), also run clippy for the other targets, see [docs/onboarding.md](docs/onboarding.md), section 5.

If you changed dependencies, run `cargo deny check` (Cargo) or `pnpm audit` (npm).

Keep a pull request focused on one change. For larger changes, open an issue first so the approach can be discussed.

## Project rules

- **Read-only.** The tool never modifies agent configuration or any other file of the scanned system.
- **Scanners access the system only through `ScanContext`.** Never use `std::env` or the real home directory directly. Paths are target paths of the scanned system (`analysis::path::TargetPath`); files are read via `context.host_path(...)`, usually through the helpers in `crates/inspector-core/src/read.rs`. This is what makes fixtures for every platform testable on every machine.
- **Scanners never fail as a whole.** Problems become `Diagnostic`s.
- **User-facing texts are messages, not strings.** In `inspector-core`, every text shown to users is a `Message` with a key from the English catalog in [`crates/inspector-core/src/model/message.rs`](crates/inspector-core/src/model/message.rs). After changing the catalog, regenerate the English GUI file with `UPDATE_EXPECT=1 cargo test -p inspector-core gui_catalog` and add the German text to [`src/locales/de.json`](src/locales/de.json). The CLI is English only; the GUI is translated into German and English.
- **Fixtures contain invented data only.** No real user names, paths, tokens or configuration from your own machine in `fixtures/`.
- **Privacy: never output secrets.** Read only the configuration keys a scanner needs. Never output MCP `env`, `headers` or tokens. Values that may contain secrets go through `crates/inspector-core/src/redact.rs`. See [docs/privacy.md](docs/privacy.md).
- **Undocumented sources** are marked `heuristic` and get at most medium confidence.
- **Dependencies in `inspector-core`** are added sparingly.
- **English** for code, comments, documentation, CLI output, commit messages and pull requests.

## Commit style

- Imperative English subject line, at most 72 characters (`Add Cursor scanner`, not `Added …`).
- A blank line, then a bullet list of the changes.

```text
Add sandbox findings for Linux fixtures

- Read sandbox.filesystem from user settings
- Add fixture linux-sandbox with expected.json
- Document the new keys in docs/claude-code-scanner.md
```

When a feature or phase is done, update `CHANGELOG.md` (section `Unreleased`) and, if applicable, the status in `docs/roadmap.md`.

## Security issues

Do not report vulnerabilities in public issues. Follow [SECURITY.md](SECURITY.md).

## License

The project is licensed under the [Apache License 2.0](LICENSE). By submitting a contribution, you agree that it is licensed under the same license.
