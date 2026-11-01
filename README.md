# AI Agent Access Inspector

Eine lokale, plattformübergreifende Anwendung, die zeigt, welche Ordner laut Konfigurationen, Projektdateien und erkennbaren Einstellungen für lokale KI-Agenten freigegeben oder potenziell erreichbar sind.

Der erste unterstützte Agent ist **Claude Code**. Weitere Agenten folgen als eigenständige Scanner.

Die Anwendung arbeitet vollständig lokal, verändert keine Agentenkonfiguration und überträgt keine Daten.

> **Status:** frühe Entwicklung (Phase 0). Es gibt noch keine Scan-Funktion.

## Entwicklung

Voraussetzung: [Rust](https://rustup.rs/) (stable). Die benötigten Komponenten werden über `rust-toolchain.toml` festgelegt.

```bash
cargo build
cargo test
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo run -p inspector-cli -- --version
```

### Aufbau

| Pfad | Inhalt |
|---|---|
| `crates/inspector-core` | Datenmodell, Pfadanalyse, Scanner (ohne Tauri) |
| `crates/inspector-cli` | Kommandozeilenprogramm `ai-agent-access-inspector` |
| `docs/` | Roadmap und Dokumentation |

Planung und Stand: [docs/roadmap.md](docs/roadmap.md).

## Lizenz

[Apache-2.0](LICENSE)
