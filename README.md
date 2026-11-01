# AI Agent Access Inspector

A local, cross-platform application that shows which folders local AI agents are granted or can potentially reach, according to their configuration, project files and detectable settings.

The first supported agent is **Claude Code**. More agents will follow as independent scanners.

The application works entirely locally, never changes agent configuration and transmits no data.

> **Status:** early development. The command line tool scans the configuration of Claude Code; a desktop GUI (German, English) is in development and can be started from source with `pnpm install && pnpm tauri dev`.

## Usage

```bash
cargo run -p inspector-cli -- scan                      # JSON report of this system
cargo run -p inspector-cli -- scan --format markdown    # readable report
cargo run -p inspector-cli -- list-scanners
```

The report lists every folder or rule that grants, restricts or suggests file access, with its source file, an explanation and warnings for broad or sensitive grants. It reads configuration files only; nothing is changed or sent anywhere.

Known limitations of the Claude Code scanner (settings given only on the command line, server-managed policies, plugins and more) are listed in [docs/claude-code-scanner.md](docs/claude-code-scanner.md#11-known-limitations).

## Development

Requirement: [Rust](https://rustup.rs/) (stable). The required components are pinned in `rust-toolchain.toml`.

```bash
cargo build
cargo test
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo run -p inspector-cli -- --version
```

### Layout

| Path | Content |
|---|---|
| `crates/inspector-core` | data model, path analysis, scanners (without Tauri) |
| `crates/inspector-cli` | command line program `ai-agent-access-inspector` |
| `src-tauri/`, `src/` | desktop GUI (Tauri, React) |
| `docs/` | roadmap and documentation |

New to the project? [docs/onboarding.md](docs/onboarding.md) explains the build setup, tests and CI.

Planning and status: [docs/roadmap.md](docs/roadmap.md).

## Security

Please report vulnerabilities privately, see [SECURITY.md](SECURITY.md).

## License

[Apache-2.0](LICENSE)
