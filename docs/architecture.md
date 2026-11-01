# Architecture

## 1. Overview

```text
┌──────────────────┐   ┌──────────────────┐
│  inspector-cli   │   │  src-tauri (GUI) │   from phase 5
└────────┬─────────┘   └────────┬─────────┘
         │                      │
         └──────────┬───────────┘
                    ▼
          ┌───────────────────┐
          │  inspector-core   │   no Tauri dependency
          │                   │
          │  context          │   ScanContext: system or fixture
          │  scanner          │   AgentScanner, run_scan, registry
          │  scanners/*       │   one module per agent
          │  model            │   Finding, Diagnostic, ScanReport
          │  platform         │   operating system
          │  analysis         │   path analysis (phase 3)
          │  export           │   JSON, Markdown (phase 4/6)
          └───────────────────┘
```

All logic lives in `inspector-core`. CLI and GUI are thin layers: they build a `ScanContext`, select scanners, call `run_scan` and present the `ScanReport`.

## 2. Scan flow

1. **Build the context.** `ScanContext::from_system()` or `ScanContext::from_fixture(dir)`. The caller applies optional overrides (home directory, current project).
2. **Select scanners.** `default_scanners()` returns all scanners for real agents, `find_scanner(id)` a specific one, including the demo scanner.
3. **Scan.** `run_scan` calls `detect` for each scanner and `scan` only for detected agents. A crashing scanner is caught and reported as diagnostic `internal`.
4. **Analysis** (from phase 3). Path normalisation, existence checks, deduplication and warnings across all findings.
5. **Output.** The `ScanReport` is serialised (JSON, later Markdown) or handed to the GUI.

## 3. Scanners

```rust
pub trait AgentScanner: Send + Sync {
    fn metadata(&self) -> ScannerMetadata;
    fn detect(&self, context: &ScanContext) -> DetectionResult;
    fn scan(&self, context: &ScanContext) -> ScanOutput;
}
```

- `scan` returns `ScanOutput` (findings, diagnostics, summary). `run_scan` adds metadata and detection result, so every scanner yields the same result structure.
- Scanners are stateless static values registered in `scanners/mod.rs`: `REAL` for real agents, `EXTRA` for scanners that only run when selected explicitly (currently `demo`).
- Scanners read only through the `ScanContext`. They build system-wide paths (e.g. managed settings) with `context.system_path("/etc/…")`, so that fixture scans can redirect them.
- Scanners do not normalise paths globally, do not deduplicate across scanners, export nothing and never change configuration.

## 4. Scan context

| Field | System scan | Fixture scan |
|---|---|---|
| `home_dir` | real home directory | `<fixture>/<home>` from `fixture.json` (default `home`) |
| `current_dir` | only if given | `<fixture>/<currentDir>` from `fixture.json` |
| `operating_system` | running system | from `fixture.json` |
| `env` | only the variables in `CAPTURED_ENV_VARS` | `env` from `fixture.json` |
| `system_root` | `None` | `<fixture>/system` |

`system_path` maps an absolute system path to the path to read. In a fixture scan, `/etc/claude-code` ends up at `<fixture>/system/etc/claude-code` and `C:\Program Files\ClaudeCode` at `<fixture>/system/C/Program Files/ClaudeCode`. This way Windows fixtures can be tested on macOS and Linux too.

Example `fixture.json`:

```json
{
  "operatingSystem": "linux",
  "home": "home/alice",
  "currentDir": "home/alice/project",
  "env": { "CLAUDE_CONFIG_DIR": "/srv/claude" }
}
```

Unknown fields in `fixture.json` are an error, so that typos in fixtures are noticed.

## 5. CLI

```text
ai-agent-access-inspector scan [--agent ID]... [--format json] [--home DIR] [--cwd DIR] [--fixture DIR]
ai-agent-access-inspector list-scanners
```

Exit codes: `0` success (also with diagnostics), `1` error (e.g. unknown scanner, invalid fixture), `2` invalid arguments.

## 6. Dependencies

| Crate | Dependencies |
|---|---|
| `inspector-core` | `serde`, `serde_json` |
| `inspector-cli` | `inspector-core`, `clap`, `serde_json` |

New dependencies in the core are added sparingly, because the core is embedded in both CLI and GUI and works locally with user data.
