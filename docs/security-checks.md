# Security checks

Record of the security check that ends each phase (see [roadmap](roadmap.md), section 14). Each entry lists what was checked, how, and what remains open.

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
