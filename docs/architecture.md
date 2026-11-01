# Architektur

## 1. Überblick

```text
┌──────────────────┐   ┌──────────────────┐
│  inspector-cli   │   │  src-tauri (GUI) │   ab Phase 5
└────────┬─────────┘   └────────┬─────────┘
         │                      │
         └──────────┬───────────┘
                    ▼
          ┌───────────────────┐
          │  inspector-core   │   ohne Tauri-Abhängigkeit
          │                   │
          │  context          │   ScanContext: System oder Fixture
          │  scanner          │   AgentScanner, run_scan, Registry
          │  scanners/*       │   ein Modul pro Agent
          │  model            │   Finding, Diagnostic, ScanReport
          │  platform         │   Betriebssystem
          │  analysis         │   Pfadanalyse (Phase 3)
          │  export           │   JSON, Markdown (Phase 4/6)
          └───────────────────┘
```

Die gesamte Logik liegt in `inspector-core`. CLI und GUI sind dünne Schichten: Sie bauen einen `ScanContext`, wählen Scanner aus, rufen `run_scan` auf und stellen den `ScanReport` dar.

## 2. Ablauf eines Scans

1. **Kontext bilden.** `ScanContext::from_system()` oder `ScanContext::from_fixture(dir)`. Optionale Überschreibungen (Home-Verzeichnis, aktuelles Projekt) setzt der Aufrufer.
2. **Scanner wählen.** `default_scanners()` liefert alle Scanner für echte Agenten, `find_scanner(id)` einen bestimmten, auch den Demo-Scanner.
3. **Scannen.** `run_scan` ruft je Scanner `detect` auf und nur bei erkanntem Agenten `scan`. Ein Absturz eines Scanners wird abgefangen und als Diagnose `internal` gemeldet.
4. **Analyse** (ab Phase 3). Pfadnormalisierung, Existenzprüfung, Deduplizierung und Warnungen über alle Findings.
5. **Ausgabe.** Der `ScanReport` wird serialisiert (JSON, später Markdown) oder an die GUI übergeben.

## 3. Scanner

```rust
pub trait AgentScanner: Send + Sync {
    fn metadata(&self) -> ScannerMetadata;
    fn detect(&self, context: &ScanContext) -> DetectionResult;
    fn scan(&self, context: &ScanContext) -> ScanOutput;
}
```

- `scan` liefert `ScanOutput` (Findings, Diagnosen, Zusammenfassung). Metadaten und Erkennungsergebnis fügt `run_scan` hinzu, damit jeder Scanner dieselbe Ergebnisstruktur liefert.
- Scanner sind zustandslose statische Werte und in `scanners/mod.rs` registriert: `REAL` für echte Agenten, `EXTRA` für Scanner, die nur ausdrücklich laufen (derzeit `demo`).
- Scanner lesen ausschließlich über den `ScanContext`. Systemweite Pfade (z. B. Managed-Settings) bilden sie mit `context.system_path("/etc/…")`, damit Fixture-Scans sie umlenken können.
- Scanner normalisieren Pfade nicht global, deduplizieren nicht über Scanner hinweg, exportieren nichts und verändern keine Konfiguration.

## 4. Scan-Kontext

| Feld | Systemscan | Fixture-Scan |
|---|---|---|
| `home_dir` | echtes Home-Verzeichnis | `<fixture>/<home>` aus `fixture.json` (Standard `home`) |
| `current_dir` | nur wenn angegeben | `<fixture>/<currentDir>` aus `fixture.json` |
| `operating_system` | laufendes System | aus `fixture.json` |
| `env` | nur die Variablen aus `CAPTURED_ENV_VARS` | `env` aus `fixture.json` |
| `system_root` | `None` | `<fixture>/system` |

`system_path` bildet einen absoluten Systempfad auf den zu lesenden Pfad ab. Im Fixture-Scan landet `/etc/claude-code` unter `<fixture>/system/etc/claude-code` und `C:\Program Files\ClaudeCode` unter `<fixture>/system/C/Program Files/ClaudeCode`. So lassen sich Windows-Fixtures auch unter macOS und Linux testen.

Beispiel für `fixture.json`:

```json
{
  "operatingSystem": "linux",
  "home": "home/alice",
  "currentDir": "home/alice/projekt",
  "env": { "CLAUDE_CONFIG_DIR": "/srv/claude" }
}
```

Unbekannte Felder in `fixture.json` sind ein Fehler, damit Tippfehler in Fixtures auffallen.

## 5. CLI

```text
ai-agent-access-inspector scan [--agent ID]... [--format json] [--home DIR] [--cwd DIR] [--fixture DIR]
ai-agent-access-inspector list-scanners
```

Exit-Codes: `0` Erfolg (auch bei Diagnosen), `1` Fehler (z. B. unbekannter Scanner, ungültige Fixture), `2` falsche Aufrufparameter.

## 6. Abhängigkeiten

| Crate | Abhängigkeiten |
|---|---|
| `inspector-core` | `serde`, `serde_json` |
| `inspector-cli` | `inspector-core`, `clap`, `serde_json` |

Neue Abhängigkeiten im Kern werden sparsam aufgenommen, weil der Kern in CLI und GUI eingebunden wird und lokal mit Benutzerdaten arbeitet.
