# Changelog

Alle nennenswerten Änderungen werden hier dokumentiert. Das Format folgt [Keep a Changelog](https://keepachangelog.com/de/1.1.0/), die Versionierung [Semantic Versioning](https://semver.org/lang/de/).

## [Unreleased]

### Hinzugefügt

- Cargo-Workspace mit `inspector-core` und `inspector-cli`
- CLI-Grundgerüst mit `--version` und `--help`
- Datenmodell für Findings, Diagnosen und Scan-Berichte mit JSON-Serialisierung
- Scanner-Schnittstelle, Scan-Kontext mit System- und Fixture-Modus
- Demo-Scanner mit festen Beispiel-Findings
- CLI-Befehle `scan` (JSON-Ausgabe, `--agent`, `--home`, `--cwd`, `--fixture`) und `list-scanners`
- CI mit Formatprüfung, Clippy und Tests auf macOS, Windows und Linux
