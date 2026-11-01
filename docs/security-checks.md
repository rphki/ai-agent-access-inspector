# Security checks

Record of the security check that ends each phase (see [roadmap](roadmap.md), section 14). Each entry lists what was checked, how, and what remains open.

## Phase 7 – Full security review before the first public release (2026-11-01)

Basis: [threat-model.md](threat-model.md). All security-relevant code was reviewed again: `read.rs`, `redact.rs`, `export/anonymize.rs`, `src-tauri/src/commands/*`, capability and CSP, the frontend (rendering, clipboard, export dialog), both workflows.

| Topic | Check | Result |
|---|---|---|
| Untrusted files | `read.rs`: regular files only, size limits, session logs capped; no path from a configuration file is executed or written | unchanged since phase 4; time-of-check/time-of-use with named pipes accepted (threat model, section 5) |
| Output and exports | redaction pass, path privacy, id renumbering | acceptance test from phase 6 passes |
| IPC | seven commands, each with its own permission; no core or plugin permissions for the frontend; open and export commands take no trusted path from the frontend | review of `capabilities/default.json`, `commands/` |
| Frontend | no `dangerouslySetInnerHTML`, `eval` or remote content; strict CSP; no devtools in release builds | `grep` in `src/`, `tauri.conf.json` |
| Release workflow permissions | default `contents: read`; only the publish job writes, only for `v*` tags; the version check covers `Cargo.toml` and `package.json` | review of `release.yml` |
| Supply chain (changed) | **all actions pinned to commit SHAs**; **`persist-credentials: false`** on every checkout; **no dependency cache in release builds**; no third-party release action; `cargo deny` and `pnpm audit` pass | workflows |
| Signing and secrets | builds unsigned (maintainer decision for `0.x`, macOS ad hoc); no secrets besides `GITHUB_TOKEN`; no private keys in the repository | `release.yml`, `git grep` for key material |
| `SECURITY.md` | supported versions, unsigned builds and checksums, links to threat model and privacy | updated |

Open points, carried forward: signing and notarisation (`1.0.0`), heuristic redaction, anonymisation limited to user directories, TOCTOU in `read.rs` (all in [threat-model.md](threat-model.md), section 5).

## Phase 6 – Export and privacy (2026-11-01)

| Topic | Check | Result |
|---|---|---|
| Redaction of all output | `redact::redact_result` runs last in `scanner::run_one` over all free text (rules, raw values, message parameters, summary); covers CLI, GUI and exports. New: headers (`Authorization: Bearer …`, `X-Api-Key:`), multi-word arguments, secrets inside permission rules and diagnostics. Finding ids are hashed from the redacted rule | unit tests in `redact.rs`; fixture `secrets` with invented tokens in rules, sandbox exclusions and MCP arguments; `no_secret_from_any_fixture_appears_in_the_output` |
| Exports without secrets | every fixture × `full`/`home`/`anonymized` × JSON/Markdown: no word that `looks_like_secret`, apart from `<redacted>` | `exports_contain_no_secrets_home_or_user_names` |
| Path privacy | `home`: no home directory as a path of its own; `anonymized`: no own user name anywhere, including JSON pointers and Claude Code's dash-encoded project directories (found by this check); ids renumbered | same test; unit tests in `export/anonymize.rs` (macOS, Linux, Windows, case, both separators, prefix boundaries, `/dev/null` with user `dev`) |
| Writing exports | `export_scan` opens the save dialog in Rust; the frontend cannot choose or name a file to write. Only the content of the last scan is written | code review; frontend tests for options, cancel and error |
| Capabilities | one new command `export_scan` with its own permission; the dialog plugin gets no frontend permission | review of `capabilities/default.json` |
| Dependencies | `tauri-plugin-dialog` (MIT/Apache-2.0); `cargo deny check` and `pnpm audit` pass | `cargo deny check`, `pnpm audit` |

Open points, carried forward:

- Redaction stays heuristic (see [privacy.md](privacy.md), section 4): a secret without a recognisable format passed as a plain argument is not detected. Command lines are still shown, so that users can identify a server.
- Anonymisation covers the home directory and user directories; other personal names in paths and host names are kept (documented in privacy.md).

## Phase 5 – Tauri and GUI (2026-11-01)

| Topic | Check | Result |
|---|---|---|
| IPC commands | six read-only commands (architecture, section 8.1); none writes, deletes or changes configuration; `run_scan` takes no arguments and always scans the real system with the default scanners | code review of `src-tauri/src/commands/` |
| Capabilities | commands declared in `build.rs`, so each needs a permission; `capabilities/default.json` grants only these six to the window `main`, no `core:*` and no plugin permissions; the opener plugin is only called from Rust | review of `capabilities/default.json` and generated permissions |
| Opening paths | only paths of the stored report, exact comparison; revealing never runs anything; opening only for regular `.json`/`.md` files after resolving symlinks, otherwise reveal | unit tests in `commands/open.rs`: unreported path, `..` path, script, directory, missing file, symlink `*.json` → script |
| Content security policy | `default-src 'self'`, scripts only from the app, `connect-src` only IPC, `object-src`, `base-uri`, `form-action`, `frame-ancestors` `'none'`; `freezePrototype`; no remote URLs, no `withGlobalTauri` | review of `tauri.conf.json` |
| Rendering untrusted values | paths, rule patterns and other values from configuration files are rendered as React text (escaped); no `dangerouslySetInnerHTML`, no `eval` | code review (`grep` in `src/`) |
| No network | no updater, no remote content; the only frontend dependencies at runtime are React and `@tauri-apps/api` | review of `tauri.conf.json` and `package.json` |
| Secrets | the GUI shows only the report the core produced; redaction stays in the core (phase 4). "Copy finding as JSON" copies the same redacted finding | code review |
| Dependencies | `cargo deny check` passes. New licenses for Tauri: BSD-3-Clause, Zlib, Apache-2.0 WITH LLVM-exception, MPL-2.0 (unmodified Servo crates). `rust-version` raised to 1.88 so that `cargo update` can pick the fixed `quick-xml` and `time` (RUSTSEC-2026-0194, -0195, -0009). Four "unmaintained" advisories in Tauri's Linux tree (proc-macro-error, zvariant, zvariant_derive, zbus_names) have no safe upgrade and are ignored with a reason | `cargo deny check` |
| npm packages | `pnpm audit` passes; `minimatch` (GHSA-g3hv-75wg-m249, dev only via ESLint) lifted with an override; pnpm's minimum release age is kept | `pnpm audit` |

Open points, carried forward:

- `open_source_file` hands JSON and Markdown files to the default application of the system. What that application does with the file is outside the inspector's control.
- The ignored "unmaintained" advisories are re-checked when Tauri moves to GTK 4 bindings or zbus 6.
- The GUI is not bundled, signed or released yet (phase 7).

## Phase 4 – Claude Code scanner and CLI (2026-11-01)

| Topic | Check | Result |
|---|---|---|
| Reading untrusted files | all reads go through `read.rs`: regular files only (devices, pipes, sockets and directories are rejected after following symlinks), size limit 16 MiB, checked again after reading | tests for `/dev/zero`, directories, oversized and missing files |
| Session logs | at most the first 4 MiB are read; only `cwd` and `version` are deserialised, the rest of each line is dropped | test with extra fields; fixture `config-dir` |
| Malformed input | invalid and truncated JSON, wrong types, unknown keys, unparsable rules produce diagnostics, never a crash | fixture `malformed`; `serde_json` limits nesting depth, so deeply nested input cannot overflow the stack |
| Unbounded input | directory listings capped at 10,000 entries, session directories at 1,000, Git root search at 64 levels | code review |
| Secrets in output | `.claude.json`: only `projects` and `mcpServers` are kept, account data is dropped right after parsing. MCP `env`, `headers`, `headersHelper`, `oauth` and `url` are never output; `command` and `args` are redacted (token prefixes, long random strings, secret flags, `NAME_TOKEN=…` assignments, URLs with secret parameters) | fixture test `no_secret_from_any_fixture_appears_in_the_output` over all fixtures; unit tests in `redact.rs` |
| No changes to configuration | the scanner only opens files for reading; no write, rename or delete calls exist in `inspector-core` outside tests | code review (`grep` for `fs::write`, `remove`, `rename`, `OpenOptions`) |
| No program execution | `claude` and MCP server commands are never executed; the policy helper is only reported | code review (`grep` for `Command`) |
| No network | no networking code or dependency in the core or CLI | `cargo deny` dependency list, code review |
| Dependencies | `cargo deny check`: advisories, licenses, sources | passes |
| Release workflow | build jobs `contents: read`; only the publish job has `contents: write` and only for `v*` tags; uses `GITHUB_TOKEN` only; tag must match `Cargo.toml` version; checksums published | review of `release.yml`; see [release-process.md](release-process.md) |

Open points, carried forward:

- Redaction is heuristic. A secret without a recognisable format, passed as a plain positional argument, can still appear in a command line. The command line is shown so that users can identify the server; this trade-off is revisited in phase 6 (export).
- Error messages from the operating system (e.g. "permission denied") are passed through as message parameters; they contain the file path but no file contents.
- Release archives are not signed yet (phase 7).
