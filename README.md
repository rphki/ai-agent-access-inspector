# AI Agent Access Inspector

A local, cross-platform application that shows which folders local AI agents are granted or can potentially reach, according to their configuration, project files and detectable settings.

The first supported agent is **Claude Code**. More agents will follow as independent scanners.

The application works entirely locally, never changes agent configuration and transmits no data.

> **Status:** early development. The data model is in place; a scanner for Claude Code is next. So far only the demo scanner returns example data:
>
> ```bash
> cargo run -p inspector-cli -- scan --agent demo
> ```

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
| `docs/` | roadmap and documentation |

Planning and status: [docs/roadmap.md](docs/roadmap.md).

## License

[Apache-2.0](LICENSE)
