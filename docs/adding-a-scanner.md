# Adding a scanner

This guide walks through adding support for a new AI agent. A scanner detects the agent, reads its configuration tolerantly and translates it into findings: which paths the agent (or one of its MCP servers) may read or write, and under which rule. Everything after that (path analysis, deduplication, redaction, export, GUI) is shared and needs no changes.

The examples use an invented agent "Example Agent" with the id `example-agent` and a settings file `~/.example/settings.json`:

```json
{ "allowedDirectories": ["~/projects", "/srv/data"] }
```

Read [CONTRIBUTING.md](../CONTRIBUTING.md) first; its rules apply throughout. The real reference implementation is the Claude Code scanner in `crates/inspector-core/src/scanners/claude_code/`; the smallest complete scanner is `crates/inspector-core/src/scanners/demo.rs`.

## 1. Write a specification

Before writing code, write `docs/<agent>-scanner.md`, modelled on [claude-code-scanner.md](claude-code-scanner.md). It is the reviewed reference for what the scanner reads and why. At least:

- **Status and scope:** agent version examined, sources used, what is out of scope.
- **How the agent controls file access:** working directory, allow/deny rules, sandbox, MCP servers.
- **Configuration locations** per operating system and level (managed, user, project), including environment variables that move them.
- **Evaluated keys** and their meaning, **precedence and merging** between levels.
- **Rule syntax and path resolution:** what relative paths and `~` are resolved against.
- **Source classification:** for each source, whether it is *documented* by the vendor or only observed (*heuristic*), with links.
- **Privacy while reading:** which keys are read, which are never read.
- **Known limitations** and **fixture requirements** (which cases the fixtures must cover).

Look at your own configuration only for structure and key names, never copy values.

## 2. Implement the scanner

Create `crates/inspector-core/src/scanners/example_agent.rs` (or a directory `example_agent/` with `mod.rs` for a larger scanner, as `claude_code/` does).

### The trait

Every scanner implements `AgentScanner` from `crates/inspector-core/src/scanner.rs`:

```rust
pub trait AgentScanner: Send + Sync {
    fn metadata(&self) -> ScannerMetadata;
    fn detect(&self, context: &ScanContext) -> DetectionResult;
    fn scan(&self, context: &ScanContext) -> ScanOutput;
}
```

The runner (`run_scan`) calls `detect` first and `scan` only if the status is `DetectionStatus::Configured`. It then runs the path analysis on the findings and finally redacts the whole result. A panicking scanner becomes an internal diagnostic, but a scanner should not rely on that: it never fails as a whole and turns every problem into a `Diagnostic`.

### Access the system only through `ScanContext`

`ScanContext` (`crates/inspector-core/src/context.rs`) is everything a scanner may know about the system. It is created either from the running system or from a fixture, so the same code is tested for macOS, Windows and Linux on any machine.

| Use | Not |
|---|---|
| `context.home()` → `Option<TargetPath>` | `std::env::home_dir()` |
| `context.env_var("APPDATA")` → `Option<&str>` | `std::env::var` |
| `context.operating_system` | `cfg!(target_os = ...)` |
| `TargetPath::parse(os, s)`, `path.join("a/b")` | `std::path::Path` logic (it follows the host, not the scanned system) |
| `read::read_json(context, &path.to_string())` | `std::fs::read_to_string` |

Only the environment variables listed in `CAPTURED_ENV_VARS` (`context.rs`) are visible. If your agent needs another one, add it there and say why in the pull request.

Read files only with the helpers in `crates/inspector-core/src/read.rs`. They map the target path to the host with `context.host_path(...)`, open only regular files, enforce a size limit and return problems as diagnostics:

```rust
pub fn read_json(context: &ScanContext, target: &str) -> JsonFile;   // Missing | Parsed(Value) | Failed(Diagnostic)
pub fn read_text(context: &ScanContext, target: &str) -> Result<Option<String>, Box<Diagnostic>>;
pub fn exists(context: &ScanContext, target: &str) -> bool;
pub fn is_dir(context: &ScanContext, target: &str) -> bool;
pub fn list_dir(context: &ScanContext, target: &str) -> Vec<String>;
```

### Metadata and detection

```rust
use crate::analysis::path::TargetPath;
use crate::analysis::resolve::resolve_path;
use crate::context::ScanContext;
use crate::model::{
    AccessKind, Confidence, DetectionResult, DetectionStatus, Diagnostic, DiagnosticCode,
    DiagnosticLevel, Finding, FindingScope, FindingSource, FindingStatus, FindingTarget, Message,
    PatternKind, RuleEffect, ScanOutput, ScannerMetadata, SourceDocumentation,
};
use crate::read::{self, JsonFile};
use crate::scanner::AgentScanner;

const PARSER_VERSION: &str = "1";
const CONFIG_KIND: &str = "example-agent/settings";

pub struct ExampleAgentScanner;

/// `~/.example/settings.json` as a target path.
fn settings_path(context: &ScanContext) -> Option<TargetPath> {
    context.home()?.join(".example/settings.json").ok()
}

impl AgentScanner for ExampleAgentScanner {
    fn metadata(&self) -> ScannerMetadata {
        ScannerMetadata {
            id: "example-agent".into(),
            name: "Example Agent".into(),
            description: "Settings of Example Agent".into(),
            parser_version: PARSER_VERSION.into(),
        }
    }

    fn detect(&self, context: &ScanContext) -> DetectionResult {
        let Some(settings) = settings_path(context) else {
            return DetectionResult::not_detected();
        };
        if !read::exists(context, &settings.to_string()) {
            return DetectionResult::not_detected();
        }
        DetectionResult {
            status: DetectionStatus::Configured,
            evidence: vec![settings.to_path_buf()],
            version: None,
        }
    }

    fn scan(&self, context: &ScanContext) -> ScanOutput {
        // see below
    }
}
```

- `id` is stable and used on the command line (`--agent example-agent`) and in finding ids. `name` is shown to users.
- Increase `PARSER_VERSION` when the parsing logic changes; it is recorded in every finding's source.
- `CONFIG_KIND` names the kind of file, `<agent-id>/<kind>`, e.g. `claude-code/settings`.
- Detection only looks for configuration (`Configured`), not for the installed program. `evidence` lists the paths that led to the detection; set `version` if the configuration reveals it.

### Findings

A finding is built with `Finding::builder(&meta, scope, target, source)` and the `FindingBuilder` methods (`model/finding.rs`). Unset fields keep neutral defaults (`effect: NotApplicable`, `access: Unknown`, `status: Unknown`, `confidence: Low`, explanation `finding.noExplanation`); the id is computed in `build()`.

```rust
    fn scan(&self, context: &ScanContext) -> ScanOutput {
        let meta = self.metadata();
        let mut out = ScanOutput::default();
        let (Some(home), Some(settings)) = (context.home(), settings_path(context)) else {
            return out;
        };
        let value = match read::read_json(context, &settings.to_string()) {
            JsonFile::Parsed(value) => value,
            JsonFile::Missing => return out,
            JsonFile::Failed(diagnostic) => {
                out.diagnostics.push(diagnostic);
                return out;
            }
        };
        // Read only the key this scanner needs; nothing else is kept.
        let Some(entries) = value.get("allowedDirectories").and_then(|v| v.as_array()) else {
            return out;
        };
        for (index, entry) in entries.iter().enumerate() {
            let pointer = format!("/allowedDirectories/{index}");
            let Some(raw) = entry.as_str() else {
                out.diagnostics.push(
                    Diagnostic::new(
                        DiagnosticLevel::Warning,
                        DiagnosticCode::UnexpectedType,
                        Message::new("diagnostic.unexpectedType")
                            .arg("pointer", &pointer)
                            .arg("expected", "string"),
                    )
                    .file(settings.to_path_buf())
                    .pointer(&pointer),
                );
                continue;
            };
            let source =
                FindingSource::new(CONFIG_KIND, SourceDocumentation::Documented, PARSER_VERSION)
                    .file(settings.to_path_buf())
                    .pointer(&pointer)
                    .raw_value(raw);
            let target = match resolve_path(raw, &home, None) {
                Ok(path) => FindingTarget::path(raw, PatternKind::Directory, path.to_path_buf()),
                Err(_) => FindingTarget::unresolved(raw, PatternKind::Unparsed),
            };
            out.findings.push(
                Finding::builder(&meta, FindingScope::User, target, source)
                    .effect(RuleEffect::Allow)
                    .access(AccessKind::ReadWrite)
                    .status(FindingStatus::Configured)
                    .confidence(Confidence::High)
                    .explanation(Message::new("finding.exampleAgent.allowedDirectory"))
                    .build(),
            );
        }
        out
    }
```

Choosing the values:

- **Target:** `FindingTarget::path(raw, kind, base_path)` when the path could be resolved to an absolute target path (`analysis::resolve::resolve_path` handles `~` and relative paths; `expand_env` expands variables first). Otherwise `FindingTarget::unresolved(raw, kind)`, e.g. for a whole tool (`PatternKind::Everything`). For relative patterns that are resolved later, set the anchor with `FindingSource::anchor(PathAnchor::...)`.
- **Source documentation** (`SourceDocumentation`): `Documented` if the vendor documents the source, `Heuristic` if it is only observed in installations, `Derived` if the inspector computes the finding rather than reading it (then no `file`).
- **Confidence:** heuristic sources get **at most `Medium`**. `High` only for documented, unambiguous values.
- **Status:** `Configured` (written in a configuration), `Observed` (seen in project or session data), `Potential` (reachable through process context, e.g. an unsandboxed shell), `Blocked`.
- **Scope:** the level the value comes from (`Managed`, `User`, `Project`, `ProjectLocal`, `Session`, `Runtime`, `OperatingSystem`).
- **Subject:** `FindingSubject::Agent` is the default; use `.subject(FindingSubject::McpServer { name })` for paths an MCP server can reach.
- **Warnings:** attach with `.warning(Warning::new(kind, severity, message))` for scanner-specific risks. Generic path warnings (home directory, filesystem root, sensitive locations, missing paths, symlinks) are added by the path analysis; do not duplicate them.
- **Summary:** key facts such as the permission mode go into `out.summary` as `SummaryEntry::new(key, label, value)`.

See [findings-model.md](findings-model.md) for every field and the JSON format.

## 3. Add messages and the German text

All user-facing text in `inspector-core` is a `Message` with a key from `CATALOG` in [`crates/inspector-core/src/model/message.rs`](../crates/inspector-core/src/model/message.rs), never a plain string. Reuse existing keys where the meaning is the same (e.g. `diagnostic.unexpectedType`, `diagnostic.invalidJson`, `value.raw`); add new keys for agent-specific explanations.

1. Add the English text to `CATALOG`. The catalog must stay **sorted by key**; keys and placeholders consist of ASCII letters, digits and dots. Placeholders are `{name}`:

   ```rust
   (
       "finding.exampleAgent.allowedDirectory",
       "Allowed directory: Example Agent can read and write here without a prompt.",
   ),
   ```

2. Use it with `Message::new("finding.exampleAgent.allowedDirectory")`; fill placeholders with `.arg("name", value)`. For technical values shown as they are (mode names, counts) use `Message::raw(value)`. Unknown keys panic in debug builds.
3. Regenerate the English GUI file: `UPDATE_EXPECT=1 cargo test -p inspector-core gui_catalog` (updates `src/locales/en.json`).
4. Add the German text with the same key and placeholders to [`src/locales/de.json`](../src/locales/de.json).

## 4. Register the scanner

In [`crates/inspector-core/src/scanners/mod.rs`](../crates/inspector-core/src/scanners/mod.rs), declare the module and add the scanner to `REAL` (runs by default) or, while it is experimental, to `EXTRA` (runs only with `--agent <id>`):

```rust
pub mod claude_code;
pub mod demo;
pub mod example_agent;

pub(crate) const REAL: &[&dyn AgentScanner] = &[
    &claude_code::ClaudeCodeScanner,
    &example_agent::ExampleAgentScanner,
];
```

The CLI (`list-scanners`, `scan --agent`) and the GUI pick it up from there. The unit test `scanner_ids_are_unique` checks that the id is not taken.

## 5. Fixtures

Fixtures are simulated systems: a manifest and a directory tree that `ScanContext::from_fixture` uses instead of the real filesystem. They contain **invented paths and values only**.

```text
fixtures/example-agent/linux-basic/
├── fixture.json
├── expected.json            generated, then reviewed
└── system/                  stands in for the filesystem root
    └── home/alice/
        ├── .example/settings.json
        └── projects/
```

`fixture.json` describes the simulated system. `home` and `currentDir` are absolute paths of that system; `env` sets the captured environment variables:

```json
{
  "operatingSystem": "linux",
  "home": "/home/alice",
  "currentDir": "/home/alice/projects",
  "env": {}
}
```

`operatingSystem` is `macos`, `windows` or `linux`. Windows paths work on every host: the drive letter becomes a directory, so `C:\Users\alice` lives in `system/C/Users/alice`.

Steps:

1. Create fixtures for the cases in your specification: at least one per operating system, *not detected*, malformed JSON and unexpected types, and a fixture with invented secrets that must not appear in the output. Use the existing `fixtures/claude-code/*` as examples.
2. Add a test `crates/inspector-core/tests/example_agent_fixtures.rs`, modelled on [`claude_code_fixtures.rs`](../crates/inspector-core/tests/claude_code_fixtures.rs): change the fixture directory and the scanner id. Keep its three tests (expectations, reproducibility, no secrets in the output) and list your invented secrets in the last one.
3. Generate the expectations and **review the diff** line by line: is every finding, scope, confidence and diagnostic what the specification says?

   ```bash
   UPDATE_EXPECT=1 cargo test -p inspector-core --test example_agent_fixtures
   git diff fixtures/
   ```

4. Run the test without `UPDATE_EXPECT` to confirm it passes. A fixture can also be scanned from the CLI:

   ```bash
   cargo run -p inspector-cli -- scan --agent example-agent --fixture fixtures/example-agent/linux-basic --format markdown
   ```

Add unit tests next to the code for parsing details that a fixture would make hard to see.

## 6. Privacy checklist

- [ ] Only the keys the scanner needs are read; everything else is dropped right after parsing (see `ClaudeJson` in the Claude Code scanner).
- [ ] No `env`, `headers`, tokens, OAuth data or URLs with credentials are output, not even in `raw_value`, `excerpt` or message parameters.
- [ ] Command lines and arguments that may contain secrets go through `redact::redact_args`; free text that may contain secrets through `redact::redact_text`. The runner additionally redacts the whole result, but a scanner must not rely on that for values it never needed.
- [ ] Session logs or other large files are read with the limits in `read.rs` (`first_json_line`, `MAX_LOG_PREFIX`), not completely.
- [ ] Fixtures contain only invented values; the fixture test checks that none of the invented secrets appears in the output.
- [ ] [docs/privacy.md](privacy.md) lists the new sources and what is read from them.

## 7. Documentation

- The specification `docs/<agent>-scanner.md` matches the implementation.
- [docs/privacy.md](privacy.md): sources and keys read.
- [docs/roadmap.md](roadmap.md): status of the scanner.
- `CHANGELOG.md`: an entry under `Unreleased`.
- `README.md`: the list of supported agents, if there is one.

## 8. Checks

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
pnpm typecheck && pnpm lint && pnpm test
```

If you added a dependency, also run `cargo deny check`. Then open a pull request as described in [CONTRIBUTING.md](../CONTRIBUTING.md).
