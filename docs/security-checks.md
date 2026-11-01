# Security checks

Record of the security check that ends each phase (see [roadmap](roadmap.md), section 14). Each entry lists what was checked, how, and what remains open.

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
