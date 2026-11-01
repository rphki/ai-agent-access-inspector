# Threat model

This document describes what the AI Agent Access Inspector protects, against whom, and how. It was written for the full security review before the first public release (phase 7) and is updated when a new scanner, command or distribution channel is added. Results of the per-phase checks are recorded in [security-checks.md](security-checks.md); privacy properties are described in [privacy.md](privacy.md).

## 1. System overview

```text
 configuration files ──read──▶ inspector-core ──report──▶ CLI (stdout)
 (untrusted input)             scanners, analysis,  └───▶ GUI backend (Tauri commands)
                               redaction, export              │ IPC (JSON)
                                                              ▼
                                                     GUI frontend (web view)
                                                              │ user action
                                                              ▼
                                       file manager / default app / export file
```

Releases are built by GitHub Actions from tagged commits and downloaded by users from GitHub Releases.

## 2. Assets

| Asset | Why it matters |
|---|---|
| Secrets in agent configuration (MCP `env`/`headers`, tokens in arguments or rules, account data in `~/.claude.json`) | disclosure gives access to external services |
| Paths and user names | reveal personal information, projects, the system layout |
| The user's system | the tool runs with the user's rights and must not become a way to run code or change files |
| Agent configuration | must never be changed (the tool's promise) |
| Integrity of released binaries | users run them with their rights |

## 3. Actors

- **Crafted configuration files:** a project folder from someone else (e.g. a cloned repository with `.claude/settings.json` or `.mcp.json`) contains hostile values: huge files, deeply nested JSON, special files, symlinks, strings that try to look like paths or markup.
- **Hostile content in the frontend:** values from configuration files are displayed in the web view and could try to inject markup or scripts.
- **A local attacker with write access to the user's files.** Such an attacker already acts with the user's rights; the tool only must not make things worse (e.g. hang or run programs).
- **A compromised dependency or GitHub Action** in the build or release pipeline.
- **A person who receives a shared report** (export) and should not learn secrets or, if requested, the user's identity.

Out of scope: an attacker with administrator rights on the user's machine, side channels, and the security of the scanned agents themselves (reported to their vendors).

## 4. Trust boundaries, threats and mitigations

### 4.1 Configuration files → core

| Threat | Mitigation | Evidence |
|---|---|---|
| Resource exhaustion through huge files, devices (`/dev/zero`), pipes | all reads through `read.rs`: regular files only, 16 MiB limit checked before and after reading, session logs at most 4 MiB, directory listings capped | tests in `read.rs`; [security-checks.md](security-checks.md), phase 4 |
| Stack exhaustion through deeply nested JSON | `serde_json` limits recursion depth | fixture `malformed` |
| Crash on malformed input | scanners never fail as a whole; problems become diagnostics; a panicking scanner is caught in `scanner::run_one` | fixture `malformed`, `scanner.rs` tests |
| Running configured programs | `claude`, MCP servers and policy helpers are never executed; no `std::process` use in the core | code review |
| Changing configuration | no write, rename or delete calls in `inspector-core` outside tests | code review |
| Symlinks pointing elsewhere | followed for reading but reported (`symlinkTarget`, warning `symlinkElsewhere`) | analysis tests |

### 4.2 Core → output (CLI, GUI, exports)

| Threat | Mitigation | Evidence |
|---|---|---|
| Secrets in output | only needed keys are read; `env`, `headers`, `oauth`, remote URLs never output; `redact::redact_result` redacts all free text as the last step of every scan; ids hashed from redacted values | fixture `secrets`, `no_secret_from_any_fixture_appears_in_the_output`, `exports_contain_no_secrets_home_or_user_names` |
| Identity in shared reports | path privacy `home` / `anonymized`, including JSON pointers and encoded project directories; ids renumbered | tests in `export/anonymize.rs`, same acceptance test |
| Redaction misses a secret | documented as heuristic; command lines stay visible on purpose so that users can identify servers | [privacy.md](privacy.md), section 4 |

### 4.3 Frontend ↔ GUI backend (IPC)

| Threat | Mitigation | Evidence |
|---|---|---|
| Script injection through displayed values | React renders values as text; no `dangerouslySetInnerHTML`, no `eval`; CSP `script-src 'self'`, no remote sources, `object-src 'none'`; `freezePrototype` | `tauri.conf.json`, code review of `src/` |
| A compromised frontend calls powerful APIs | the capability grants only the app's seven commands; no core, file system, shell, dialog or opener permissions for the frontend | `capabilities/default.json`, generated permissions |
| Opening arbitrary paths or programs | open commands accept only paths from the last report (exact match); revealing never runs anything; opening only regular `.json`/`.md` files after resolving symlinks | tests in `commands/open.rs` |
| Writing arbitrary files | `export_scan` takes no path; the save dialog is opened by the backend and the user picks the file | `commands/mod.rs` |

### 4.4 Build and release pipeline

| Threat | Mitigation | Evidence |
|---|---|---|
| Vulnerable or malicious dependency | `cargo deny` (advisories, licenses, sources: crates.io only) on every CI run and weekly; `pnpm audit`; Dependabot; exact versions in `package.json`; lockfiles with `--locked` / `--frozen-lockfile`; pnpm does not run install scripts of dependencies unless allowed; pnpm's minimum release age for new versions | `ci.yml`, `deny.toml` |
| A moved or compromised action tag | all actions pinned to commit SHAs; no third-party release action | workflows |
| Token misuse | workflows default to `contents: read`; only the publish job has `contents: write`, only for `v*` tags; `persist-credentials: false` on checkouts; CI runs on `pull_request`, never `pull_request_target`; no secrets besides `GITHUB_TOKEN` | workflows, [release-process.md](release-process.md) |
| Cache poisoning | release builds use no dependency or build cache | `release.yml` |
| Tampered downloads | `SHA256SUMS` in every release; builds are not signed yet, so checksums prove integrity but not origin | [release-process.md](release-process.md) |

## 5. Open points and accepted risks

- **Unsigned builds** until `1.0.0`: users must bypass Gatekeeper/SmartScreen. Mitigated by checksums and documentation; signing is planned (roadmap, section 17.4).
- **Time-of-check/time-of-use in `read.rs`:** a file can be replaced by a named pipe between the regular-file check and opening it, which would block that scan. This needs write access to the user's configuration directories (an attacker who already has the user's rights) and causes no disclosure; accepted.
- **Heuristic redaction** (see 4.2) and **anonymisation limited to user directories** ([privacy.md](privacy.md), section 5).
- **Opening configuration files** hands them to the system's default application for `.json`/`.md`; what that application does is outside the tool's control.
- **`style-src 'unsafe-inline'`** in the CSP: allows inline styles (needed by the web view's own styling); scripts remain restricted to the app.
