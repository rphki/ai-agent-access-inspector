# Roadmap: AI Agent Access Inspector

This roadmap is the project's authoritative planning document. It covers goals, requirements, architecture and implementation order. The original implementation plan is in [`plan-initial.md`](plan-initial.md) and is of historical interest only.

## 1. Goal

A local, cross-platform desktop application shows which folders local AI agents are granted or can potentially reach, according to their configuration, project files and detectable settings.

The first supported agent is **Claude Code**. The architecture allows Claude Desktop, Codex, ChatGPT, Cursor and other applications to be added later as independent scanners.

The application:

- runs on macOS, Windows and Linux,
- works entirely locally,
- needs no user account and no backend,
- never changes agent configuration,
- transmits no file paths,
- explains for every finding where the information comes from,
- distinguishes between configured and merely potential access.

## 2. Decisions

| Topic | Decision |
|---|---|
| Repository | public GitHub repository [`rphki/ai-agent-access-inspector`](https://github.com/rphki/ai-agent-access-inspector) |
| Local folder | renamed from `agent-access-inspector` to `ai-agent-access-inspector` |
| License | Apache-2.0 |
| Project structure | Cargo workspace with separate crates for core, CLI and Tauri |
| Order | core and CLI first (`v0.1.0`), GUI afterwards (`v0.2.0`) |
| Research | Claude Code research before the data model, because it determines the model's shape |
| MCP servers | scanned in the MVP |
| Claude Code versions | only the currently documented configuration format is supported; older formats are not specifically evaluated but never cause an abort (see 8.1) |
| Language | English for documentation, code and commits (see `CLAUDE.md`) |
| Internationalisation | only the GUI is translated (German and English); the CLI is English only. The core emits user-facing texts as message keys with parameters plus an English default text (see 12.5) |
| Security | automated dependency audit in CI from phase 3.5; a security check per phase from phase 4; a full security review before the first public release (see 14) |
| Approach | phase by phase; acceptance check and alignment after each phase |

## 3. Technology stack

- **Core and scanners:** Rust (crate `inspector-core`, no Tauri dependency)
- **CLI:** Rust (crate `inspector-cli`)
- **Desktop framework:** Tauri 2
- **Frontend:** React with TypeScript
- **Frontend build system:** Vite
- **Frontend package manager:** pnpm
- **Backend tests:** Rust unit and integration tests
- **Frontend tests:** Vitest and React Testing Library
- **End-to-end tests:** after the MVP
- **CI/CD:** GitHub Actions
- **Distribution:** GitHub Releases

## 4. Product terms

The application deliberately avoids the blanket term "accessible".

### 4.1 Finding status

- **Configured:** the path or rule is explicitly written in a configuration.
- **Observed:** the path was found in stored project or session information.
- **Potential:** the path could be reachable because of the process context or operating system rights (see 4.4).
- **Blocked:** the path is configured but currently not reachable.
- **Unknown:** there is not enough information for a reliable assessment.

### 4.2 Scope

- **Managed:** system-wide, administratively managed settings (highest priority)
- **User**
- **Project**
- **Project-local**
- **Session**
- **Runtime**
- **Operating system**

### 4.3 Rule effect

- **Allow:** permits access without a prompt.
- **Ask:** permits access after a prompt.
- **Deny:** forbids access.

Deny rules do not grant access. They are still listed as findings because they restrict other findings and are needed to detect conflicting rules.

### 4.4 Meaning of "Potential"

Without a sandbox, an agent can reach everything the user can reach through shell tools. Instead of listing every folder as `Potential`, the scanner produces one summarising finding, e.g. "shell tool without sandbox – effectively user rights". A configured sandbox narrows this finding and produces findings for the areas it allows instead.

## 5. Project structure

```text
ai-agent-access-inspector/
├── .github/
│   ├── workflows/
│   │   ├── ci.yml
│   │   └── release.yml
│   ├── dependabot.yml
│   └── ISSUE_TEMPLATE/
├── Cargo.toml                     # workspace
├── deny.toml                      # cargo deny configuration
├── CLAUDE.md                      # guidance for Claude Code
├── crates/
│   ├── inspector-core/            # no Tauri dependency
│   │   └── src/
│   │       ├── model/             # Finding, Diagnostic, ScanReport
│   │       ├── analysis/          # path analysis, deduplication, risk
│   │       ├── platform/          # operating system specifics
│   │       ├── scanners/
│   │       │   └── claude_code/
│   │       ├── export/            # JSON, Markdown, anonymisation
│   │       └── lib.rs
│   └── inspector-cli/             # CLI diagnostic mode
│       └── src/main.rs
├── src-tauri/                     # thin layer: Tauri commands
│   ├── src/
│   │   ├── commands/
│   │   ├── lib.rs
│   │   └── main.rs
│   ├── capabilities/
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                           # React frontend
│   ├── components/
│   ├── pages/
│   ├── hooks/
│   ├── types/
│   └── App.tsx
├── fixtures/
│   └── claude-code/
│       ├── macos/
│       ├── linux/
│       ├── windows/
│       └── malformed/
├── docs/
│   ├── roadmap.md
│   ├── onboarding.md              # build setup, tests, CI
│   ├── security-checks.md         # record of the per-phase security checks
│   ├── plan-initial.md            # historical, German
│   ├── architecture.md
│   ├── findings-model.md
│   ├── claude-code-scanner.md
│   ├── adding-a-scanner.md
│   ├── privacy.md
│   └── release-process.md
├── package.json
├── pnpm-lock.yaml
├── README.md
├── CONTRIBUTING.md
├── SECURITY.md
├── LICENSE
└── CHANGELOG.md
```

`src-tauri/` and `src/` are created in phase 5.

## 6. Data model

The core provides an agent-independent model (`crates/inspector-core/src/model/`). The full description including the JSON format is in [`findings-model.md`](findings-model.md).

Key elements of a finding:

| Field | Content |
|---|---|
| `subject` | the agent itself or one of its MCP servers |
| `target` | raw value, pattern kind (`exactPath`, `directory`, `glob`, `everything`, `unparsed`), base path, display path |
| `effect` | `allow`, `ask`, `deny`, `notApplicable` |
| `access`, `status`, `scope`, `confidence` | see section 4 |
| `source` | file, configuration kind, classification (`documented`, `heuristic`, `derived`), JSON pointer, raw value, anchor for relative patterns, parser version, excerpt |
| `pathState` | existence, path kind, symlink target (filled in by the path analysis) |
| `warnings` | kind, severity, message; "not effective" is a warning kind with a reason, not a status of its own |

Every finding has a stable `id`, computed from agent, subject, source and raw value.

Besides findings, a scan returns **diagnostics** (e.g. invalid JSON, unreadable file, unknown key) and a **summary** per agent (e.g. permission mode, sandbox). Diagnostics never abort a scan. The overall report (`ScanReport`) carries a format version.

## 7. Scanner interface and scan context

### 7.1 Scanner

Every agent gets its own scanner.

```rust
pub trait AgentScanner: Send + Sync {
    fn metadata(&self) -> ScannerMetadata;
    fn detect(&self, context: &ScanContext) -> DetectionResult;
    fn scan(&self, context: &ScanContext) -> ScanOutput;
}
```

`scan` returns findings, diagnostics and summary; the scan runner adds metadata and detection result. It only calls `scan` for detected agents and turns crashes of individual scanners into diagnostics. Details: [`architecture.md`](architecture.md).

A scanner is responsible for:

- detecting the agent installation,
- locating relevant configuration files,
- tolerant parsing,
- translating agent-specific settings into the common data model,
- warnings about unknown or unsupported formats.

A scanner is not responsible for:

- presentation,
- global deduplication,
- general path normalisation,
- export,
- telemetry,
- changes to the configurations it finds.

### 7.2 Scan context

Scanners must not access the real home directory or the real environment directly, only through the context.

```rust
pub struct ScanContext {
    pub home_dir: PathBuf,
    pub current_dir: Option<PathBuf>,
    pub operating_system: OperatingSystem,
    pub env: EnvSnapshot,             // selected variables only, e.g. CLAUDE_CONFIG_DIR, APPDATA
    pub system_root: Option<PathBuf>, // stands in for the filesystem root in fixture scans
}
```

All paths in the context are paths of the scanned system (target paths). Scanners read files through `context.host_path(…)`, which redirects them into the fixture directory in fixture scans. This replaces the originally planned `PlatformConfigDirs`.

This way all platform variants can be tested with fixtures on every development machine.

The core supports two modes:

- **System scan:** uses the user's actual directories and environment variables.
- **Fixture scan:** uses a controlled test directory and a predefined environment.

## 8. Claude Code scanner

### 8.1 Specification

Before implementation, the current configuration locations and levels are collected from official documentation and verifiable local installations and documented in `docs/claude-code-scanner.md`:

- the Claude Code version the specification was checked against, and the research date,
- configuration locations per operating system,
- managed, user, project and local configuration and their precedence,
- additional directories,
- allow/ask/deny rules and their pattern syntax,
- dangerous permission modes,
- sandbox settings,
- MCP server configuration,
- stored project or session information,
- known limitations of the scanner.

Only the currently documented configuration format is supported. Keys or files from older formats are not specifically interpreted; they appear as diagnostics (e.g. "unknown key") or as findings with status `Unknown`. If Claude Code changes its format, the specification is updated.

Undocumented files are explicitly marked as heuristic sources.

The specification is available in [`claude-code-scanner.md`](claude-code-scanner.md) (as of 2026-11-01). It is authoritative for the scanner; sections 8.2 to 8.4 are an overview only.

### 8.2 Sources to examine

- managed/enterprise settings per operating system
- user configuration (`~/.claude/settings.json` and possibly others)
- `.claude/settings.json` in the project
- `.claude/settings.local.json` in the project
- environment variable `CLAUDE_CONFIG_DIR` and other location-changing variables
- additional directories (e.g. `additionalDirectories`)
- filesystem-related allow/ask/deny rules
- permission modes, especially settings that skip prompts and their locks
- sandbox settings
- MCP configuration (`.mcp.json` in the project, user and project MCP entries) with path arguments
- `~/.claude.json` (project list and per-project entries) – heuristic
- `~/.claude/projects/` (stored sessions) – heuristic, status `Observed`
- session-only settings such as `--add-dir`, if they are persisted at all

`CLAUDE.md` is not treated as a folder grant. Paths it contains may at most be listed later as unverified references.

From session data only paths or project assignments are read, never contents.

### 8.3 Behaviour

The scanner:

1. detects whether Claude Code is installed or configured,
2. determines configuration locations, taking environment variables into account,
3. reads managed and user configuration,
4. determines known projects,
5. examines their project and local configurations,
6. extracts rules, additional directories, MCP servers, modes and sandbox settings,
7. passes paths and patterns to the path analysis,
8. produces findings, warnings and diagnostics,
9. produces the summarising `Potential` finding according to 4.4.

Deduplication of identical results is done by the shared analysis component.

### 8.4 Error handling

The application must not abort in these cases:

- invalid JSON,
- partially written configuration files,
- unreadable files,
- missing paths,
- unknown configuration keys,
- uninterpretable rule patterns,
- symlink loops,
- differences in letter case,
- invalid Windows or UNC paths.

A diagnostic entry is produced instead.

## 9. Path analysis

A shared analysis component in `inspector-core` handles:

- resolving `~`,
- resolving supported environment variables,
- turning relative paths into absolute ones (relative to project or configuration file, depending on the rule),
- deriving the base path from rule patterns,
- lexical normalisation,
- optional canonical resolution,
- detecting symlinks,
- checking whether a path exists,
- checking whether it is a file or a directory,
- detecting nested grants,
- deduplication,
- platform-specific handling of letter case,
- detecting particularly broad paths.

Warnings are produced, among others, for:

- the home directory,
- the filesystem root,
- a whole drive,
- a user directory with personal subfolders,
- a path that no longer exists,
- a symlink pointing to another area,
- conflicting allow/deny rules,
- enabled dangerous permission modes,
- shell access without sandbox,
- MCP servers with broad filesystem access.

## 10. CLI diagnostic mode

A CLI is provided before the GUI:

```text
ai-agent-access-inspector scan --agent claude-code --format json
```

Options:

- alternative home directory,
- fixture directory,
- JSON output,
- Markdown output,
- verbose diagnostics.

The CLI remains after the GUI is introduced and serves as a testing and diagnostic tool.

## 11. Tauri commands

The frontend only gets clearly limited commands:

```text
list_scanners
run_scan
get_scan_details
export_scan
open_path_in_file_manager
open_source_file
get_app_info
```

The commands are a thin layer over `inspector-core`. The MVP has no commands to change or delete agent configuration. Tauri capabilities are restricted to the necessary local functions; no external network access is needed.

## 12. User interface

### 12.1 Main view

- detected agents,
- time of the last scan,
- number of findings,
- number of warnings,
- "Scan again" button.

### 12.2 Results view

The results list shows:

- agent or MCP server,
- folder or rule,
- effect,
- status,
- access kind,
- scope,
- confidence,
- warnings.

Filters:

- agent,
- status,
- effect,
- access kind,
- scope,
- warnings only,
- existing paths only.

### 12.3 Detail view

For each finding:

- normalised path,
- original value or rule pattern,
- configuration source and its classification (documented/heuristic),
- exact position in the source,
- explanation,
- confidence,
- existence and path kind,
- symlink information,
- parent or child findings,
- restricting or conflicting rules,
- possible security relevance.

Actions:

- open folder in file manager,
- open configuration file,
- copy path,
- copy finding as JSON.

### 12.4 States

Defined presentations for:

- no agent found,
- Claude Code found but no relevant configuration,
- scan running,
- scan partially successful,
- configuration unreadable,
- invalid configuration,
- scan completed,
- unexpected internal error.

The interface is accessible (keyboard operation, screen-reader-friendly labels, sufficient contrast).

### 12.5 Internationalisation

- Only the GUI is translated: German and English, chosen from the system language, switchable in the GUI.
- The CLI is English only.
- The core never produces finished sentences for the GUI. Every user-facing text (finding explanations, warning and diagnostic messages, summary labels) is a **message**: a stable key, named parameters and an English default text from a catalog in the core. The JSON format carries all three.
- The GUI translates by key and fills in the parameters; it falls back to the English text for unknown keys. The English catalog of the core is the source for the GUI's translation files.
- Paths, rule patterns, file names and other technical values are parameters and are never translated.

## 13. Export

The MVP supports JSON and Markdown.

By default the export contains full paths. Before exporting, the user can choose:

- full paths,
- show the home directory as `~`,
- anonymise user names.

Exports from the GUI use the GUI language for Markdown; exports from the CLI are English. JSON always contains message keys, parameters and English texts.

Later versions may add CSV and a machine-readable security report.

## 14. Privacy and security

Binding properties:

- no telemetry in the MVP,
- no cloud connection,
- no automatic transmission of error reports,
- no collection of file contents beyond necessary configuration files,
- no storage of complete configuration files,
- no display of tokens, API keys or environment secrets (especially `env` entries of MCP servers),
- sensitive values are redacted when findings and diagnostics are produced,
- exports happen only on explicit user action.

The repository contains `docs/privacy.md` and `SECURITY.md`.

Security work happens on three levels:

1. **Automated dependency audit** (from phase 3.5): `cargo deny` checks known vulnerabilities (RustSec), licenses and crate sources in every CI run and weekly; Dependabot proposes updates for Cargo dependencies and GitHub Actions.
2. **Security check per phase** (from phase 4): each phase lists what to check; a phase is only accepted when its check is done.
3. **Full security review** before the first public release (phase 7): threat model, review of all security-relevant code, release pipeline and signing.

## 15. Test strategy

### 15.1 Unit tests

- path normalisation
- home resolution and environment variables
- relative paths
- Windows drive paths
- UNC paths
- symlinks
- nested directories
- rule patterns and base path derivation
- deduplication
- risk classification
- redaction of sensitive values

### 15.2 Scanner tests

Fixtures (without personal data) for:

- macOS, Linux, Windows
- multiple projects
- managed, user, project and local settings
- conflicting rules
- MCP servers with path arguments
- sandbox enabled/disabled
- missing folders
- invalid JSON
- unknown configuration fields
- outdated or unknown configuration keys (produce diagnostics, no abort)

### 15.3 Frontend tests

- rendering findings
- filters
- detail view
- empty states
- warnings
- export dialog
- partially failed scan

### 15.4 Manual platform tests

Before the first stable release:

- macOS on Apple Silicon
- Windows 11
- Ubuntu LTS
- paths with spaces
- paths with umlauts and Unicode
- very long paths
- network drives or mounted volumes

## 16. Continuous integration

On every pull request `ci.yml` runs:

- Rust format check
- Clippy
- Rust tests
- TypeScript type check (from phase 5)
- ESLint (from phase 5)
- frontend tests (from phase 5)
- production build
- dependency and security audit: `cargo deny` (from phase 3.5), `pnpm audit` (from phase 5)

CI additionally runs weekly, so that newly published advisories are noticed without a code change.

Operating system matrix:

```yaml
matrix:
  os:
    - macos-latest
    - windows-latest
    - ubuntu-latest
```

Rust tests and builds run on all three systems. Frontend tests need not run three times.

## 17. GitHub Releases

### 17.1 Trigger

A release is triggered by a Git tag (`v0.1.0`, `v0.2.0`, …, `v1.0.0`).

### 17.2 Artifacts

From `v0.1.0`: CLI binaries for macOS, Windows and Linux.

Once the GUI exists:

- macOS Apple Silicon
- macOS Intel, as long as sensible
- Windows x64
- optionally Windows ARM64
- Linux x64 AppImage
- Linux x64 `.deb`

### 17.3 Workflow (`release.yml`)

1. check out the repository
2. install Rust
3. install Node.js and pnpm (once the GUI exists)
4. install platform-specific dependencies
5. run tests
6. build the CLI or Tauri application
7. create installers
8. create checksums
9. create the GitHub release
10. upload artifacts
11. generate release notes from the changelog or Git history

### 17.4 Signing

The first development phase may produce unsigned builds. Before a stable public release:

- set up macOS code signing,
- publish the macOS application notarised,
- set up a Windows code signing certificate,
- store certificates only as GitHub secrets,
- never store private keys in the repository.

## 18. Versions

| Version | Content |
|---|---|
| `0.1.0` | CLI core and Claude Code scanner |
| `0.2.0` | first Tauri GUI |
| `0.3.0` | exports and improved path analysis |
| `0.4.0` | cross-platform GitHub releases of the GUI |
| `0.5.0` | Claude Desktop scanner |
| `0.6.0` | Codex/ChatGPT scanner |
| `0.7.0` | Cursor scanner |
| `1.0.0` | stable data structure, signed releases, documented extension interface |

Until `1.0.0` the scanner interface and export format may change incompatibly.

## 19. Phases

Every phase ends with an acceptance check. Only then is the next phase planned in detail.

### Phase 0 – Foundation

- install the Rust toolchain (rustup, stable), optionally `gh`
- rename the local folder to `ai-agent-access-inspector`
- initialise the Git repository, create the GitHub repository `rphki/ai-agent-access-inspector`
- Cargo workspace with `inspector-core` and `inspector-cli`
- `LICENSE` (Apache-2.0), `README.md`, `.gitignore`, `CHANGELOG.md`
- `rustfmt`, `clippy`, first tests
- CI: format, Clippy and tests on macOS, Windows, Linux

**Acceptance:** `cargo test` passes locally and in CI on all three systems.

### Phase 1 – Claude Code research

- evaluate official documentation on settings, permissions, MCP and sandbox
- examine a local installation (structure only, no contents)
- classify every source as documented or heuristic
- record precedence and merging of levels
- derive fixture requirements

**Acceptance:** `docs/claude-code-scanner.md` documents every file to read and every evaluated key with its source.

### Phase 2 – Data model and scanner interface

- finding types according to section 6, adapted to the research
- `AgentScanner` trait, `ScanContext`, diagnostic and error model
- JSON serialisation
- fixture scanner and CLI skeleton (`scan --format json`)
- `docs/findings-model.md`, `docs/architecture.md`

**Acceptance:** the CLI prints defined findings of a fixture scanner as JSON.

### Phase 3 – Path analysis

- resolving `~`, environment variables, relative paths
- deriving base paths from rule patterns
- normalisation, symlinks, existence checks
- nesting, deduplication, risk warnings
- Windows, UNC and letter case cases

**Acceptance:** all path cases from section 15.1 are tested automatically and pass on all three systems.

### Phase 3.5 – Message keys and security groundwork

- message model: key, parameters and English default text from a catalog in the core, used for explanations, warnings, diagnostics and summary labels (see 12.5)
- `cargo deny` in CI, weekly CI run, Dependabot for Cargo and GitHub Actions
- `SECURITY.md` with the reporting process
- security checks per phase and internationalisation rules in this roadmap

**Acceptance:** no user-facing text in the core is a free-form string; `cargo deny` passes in CI.

### Phase 4 – Claude Code scanner and CLI → `v0.1.0`

- detection, configuration locations, reading all levels
- permissions, additional directories, MCP servers, modes, sandbox
- tolerant behaviour with diagnostics
- fixtures for macOS, Linux, Windows and malformed cases
- complete CLI according to section 10, including Markdown output
- release workflow for CLI binaries
- **security check:** size limits and safe handling when reading untrusted files (huge files, special files such as `/dev/zero`, symlinks), no crashes on malformed input, no secrets in output (MCP `env`/`headers`, tokens, account data from `~/.claude.json`), release workflow permissions

**Acceptance:** the scanner produces reproducible, explained findings from all fixtures; a system scan via the CLI works.

### Phase 5 – Tauri and GUI → `v0.2.0`

- Tauri 2 with React/TypeScript, Vite, pnpm
- commands according to section 11, restricted capabilities
- run scans asynchronously, report progress and error states
- open folders and source files
- main, results and detail views, filters, states according to section 12
- internationalisation of the GUI (German, English) according to 12.5
- frontend tests, extend CI with frontend checks
- **security check:** Tauri capabilities and IPC commands, content security policy, opening paths and files must not allow running arbitrary programs, `pnpm audit`

**Acceptance:** all information of a finding is understandable without raw JSON.

### Phase 6 – Export and privacy → `v0.3.0`

- JSON and Markdown export with anonymisation options according to section 13
- secret redaction
- `docs/privacy.md`
- **security check:** redaction and anonymisation verified with targeted tests, no secrets in exports

**Acceptance:** exports contain no detected API keys or credentials.

### Phase 7 – Releases and publication → `v0.4.0`

- release workflow for the GUI with platform matrix, installers and checksums
- `docs/release-process.md`
- README with screenshots, installation notes and known limitations
- `CONTRIBUTING.md`, issue templates, `docs/adding-a-scanner.md`
- **full security review:** threat model, review of all security-relevant code, release workflow permissions, signing and secret handling, update of `SECURITY.md`
- first public release

**Acceptance:** a Git tag produces downloadable artifacts for macOS, Windows and Linux; an outside person can install the application, understand findings and prepare a new scanner.

### Afterwards

More scanners (Claude Desktop, Codex/ChatGPT, Cursor) and `1.0.0` according to section 18.

## 20. Definition of done for the MVP

The MVP is done when:

- Claude Code is detected,
- managed, user and project configurations are examined,
- MCP servers with filesystem access are captured,
- found paths and rules appear with source and explanation,
- configured and potential access are shown separately,
- missing paths and broad grants produce warnings,
- no configuration is changed,
- JSON and Markdown export work,
- automated tests exist,
- builds for macOS, Windows and Linux are available via GitHub Releases,
- known limitations are clearly documented.

## 21. Not part of the MVP

- automatic changes to permissions
- revoking grants
- runtime monitoring of running agent processes
- filesystem tracing
- network monitoring
- guaranteed proof of an actual sandbox
- cloud dashboard
- user account
- telemetry
- auto-update
- plugin system with dynamically loaded third-party code
- full support for all Claude products
- evaluating plugins and plugin MCP servers
- evaluating MDM profiles and registry policies (they are only detected)

These features are only considered once the plain, traceable configuration scanner works reliably.

## 22. Open items

- `rust-version = "1.85"` in `Cargo.toml` is not verified in CI and is probably too low (`std::env::home_dir` is only undeprecated from Rust 1.86 on). Decide whether to drop the field or verify it with a CI job.

## 23. Status

| Phase | Status |
|---|---|
| 0 – Foundation | completed (CI green on macOS, Windows, Linux) |
| 1 – Claude Code research | completed ([specification](claude-code-scanner.md); open checks for phase 4 in section 14) |
| 2 – Data model | completed (demo scanner prints JSON via the CLI) |
| 3 – Path analysis | completed (target paths, resolution, disk checks, warnings, deduplication, relations) |
| 3.5 – Message keys and security groundwork | completed (message catalog, cargo deny, weekly CI, Dependabot, SECURITY.md) |
| 4 – Claude Code scanner and CLI | completed: [`v0.1.0`](https://github.com/rphki/ai-agent-access-inspector/releases/tag/v0.1.0) released 2026-11-01 ([security check](security-checks.md)) |
| 5 – Tauri and GUI | open |
| 6 – Export and privacy | open |
| 7 – Releases and publication | open |
