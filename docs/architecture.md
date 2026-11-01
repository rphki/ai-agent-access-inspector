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
          │  analysis         │   path analysis
          │  export           │   JSON, Markdown (phase 4/6)
          └───────────────────┘
```

All logic lives in `inspector-core`. CLI and GUI are thin layers: they build a `ScanContext`, select scanners, call `run_scan` and present the `ScanReport`.

## 2. Scan flow

1. **Build the context.** `ScanContext::from_system()` or `ScanContext::from_fixture(dir)`. The caller applies optional overrides (home directory, current project).
2. **Select scanners.** `default_scanners()` returns all scanners for real agents, `find_scanner(id)` a specific one, including the demo scanner.
3. **Scan.** `run_scan` calls `detect` for each scanner and `scan` only for detected agents. A crashing scanner is caught and reported as diagnostic `internal`.
4. **Analysis.** `analysis::analyze` runs over each scanner's findings: normalisation, disk checks, warnings, deduplication and relations (see section 5).
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
- Scanners read only through the `ScanContext`. They work with target paths and read files via `context.host_path(…)`, so that fixture scans can redirect them.
- Scanners do not normalise paths globally, do not deduplicate across scanners, export nothing and never change configuration.

## 4. Scan context

| Field | System scan | Fixture scan |
|---|---|---|
| `home_dir` | real home directory | `home` from `fixture.json` (absolute target path) |
| `current_dir` | only if given | `currentDir` from `fixture.json` (absolute target path) |
| `operating_system` | running system | from `fixture.json` |
| `env` | only the variables in `CAPTURED_ENV_VARS` | `env` from `fixture.json` |
| `system_root` | `None` | `<fixture>/system` |

All paths in the context are **target paths**: paths of the scanned system, which in a fixture scan is a simulated one. `host_path` maps a target path to the location to read on the machine running the inspector; `target_path` maps back (used for resolved symlinks). In a fixture scan, `/etc/claude-code` lives at `<fixture>/system/etc/claude-code` and `C:\Program Files\ClaudeCode` at `<fixture>/system/C/Program Files/ClaudeCode`. The home directory works the same way. This way Windows fixtures can be tested on macOS and Linux too.

Example `fixture.json`:

```json
{
  "operatingSystem": "linux",
  "home": "/home/alice",
  "currentDir": "/home/alice/project",
  "env": { "CLAUDE_CONFIG_DIR": "/srv/claude" }
}
```

Unknown fields in `fixture.json` are an error, so that typos in fixtures are noticed. `home` and `currentDir` must be absolute paths of the fixture's operating system.

## 5. Path analysis

`std::path` follows the rules of the host, but the scanned system may be a different one (a Windows fixture tested on Linux). The analysis therefore works on its own type, `analysis::path::TargetPath`, which follows the rules of the target operating system:

- Unix paths, Windows drive paths (`C:\…`), UNC shares (`\\server\share\…`) and verbatim paths (`\\?\C:\…`, `\\?\UNC\…`)
- lexical normalisation: `.` removed, `..` resolved (never above the root), separators unified, drive letters upper case
- rejected with an error: drive-relative paths (`C:foo`), paths without drive (`\foo`), incomplete UNC paths, device paths (`\\.\…`), invalid characters
- comparison ignores letter case on Windows and macOS (default filesystems are case-insensitive) and respects it on Linux

Building blocks for scanners:

| Module | Purpose |
|---|---|
| `resolve::expand_env` | `${VAR}` and `${VAR:-default}`; unset variables stay literal and are reported |
| `resolve::resolve_path` | `~`, `~/…` and relative paths to absolute target paths |
| `pattern::split_pattern` | splits a gitignore-style pattern into base path and kind (`exactPath`, `directory`, `glob`), detects `!` negation |
| `fs::inspect` | existence, kind and symlink target of a host path, reading metadata only |

`analyze` runs after every scanner and

1. normalises each finding's base path and display path,
2. checks the path on disk (through `host_path`) and fills `pathState`,
3. adds warnings: missing path (not for deny rules), symlink pointing outside its parent directory, and for allow/ask findings the filesystem root, whole drives or shares, the home directory, ancestors of the home directory, credential locations (`~/.ssh`, `~/.aws`, …) and personal folders (`~/Documents`, …),
4. merges findings that say the same about the same location (same agent, subject, effect, access, status and kind; globs only if written identically); the merged sources go to `additionalSources`,
5. records containment (`contains`/`containedIn`) and marks allow/ask findings fully covered by a deny rule (`overriddenBy` plus a `conflictingRules` warning); glob denies and denies for a different access kind do not count,
6. sorts each finding's warnings by severity.

Limitations:

- Only the path itself is checked for being a symlink, not its parent directories.
- Unicode normalisation (NFC/NFD, relevant on macOS) is not applied when comparing paths.
- Case-insensitivity on macOS is assumed from the default filesystem; case-sensitive volumes are not detected.
- Symlink tests run on macOS and Linux only; creating symlinks on Windows requires special privileges.

## 6. CLI

```text
ai-agent-access-inspector scan [--agent ID]... [--format json] [--home DIR] [--cwd DIR] [--fixture DIR]
ai-agent-access-inspector list-scanners
```

Exit codes: `0` success (also with diagnostics), `1` error (e.g. unknown scanner, invalid fixture), `2` invalid arguments.

## 7. Dependencies

| Crate | Dependencies |
|---|---|
| `inspector-core` | `serde`, `serde_json` |
| `inspector-cli` | `inspector-core`, `clap`, `serde_json` |

New dependencies in the core are added sparingly, because the core is embedded in both CLI and GUI and works locally with user data.
