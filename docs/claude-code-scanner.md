# Specification: Claude Code scanner

Result of phase 1 of the [roadmap](roadmap.md). This document defines which files the Claude Code scanner reads, which keys it evaluates and how it translates them into findings.

## 1. Status and scope

| | |
|---|---|
| Research date | 2026-11-01 |
| Documentation checked | refers to Claude Code up to v2.1.311 |
| Local installation examined | Claude Code 2.1.309 (macOS, via the Claude desktop app) |
| Supported format | only the currently documented one (see roadmap, section 2) |

Sources (official documentation):

- [Settings](https://code.claude.com/docs/en/settings)
- [Settings reference](https://code.claude.com/docs/en/settings-reference)
- [Managed settings](https://code.claude.com/docs/en/managed-settings)
- [Permissions](https://code.claude.com/docs/en/permissions)
- [MCP](https://code.claude.com/docs/en/mcp)
- [Managed MCP](https://code.claude.com/docs/en/managed-mcp)
- [Sandboxing](https://code.claude.com/docs/en/sandboxing)
- [Environment variables](https://code.claude.com/docs/en/env-vars)

### Source classification

- **Documented:** file and key are described in the official documentation.
- **Heuristic:** file or key is not documented, or only in passing, but was observed in a local installation. Findings from it get at most `Confidence::Medium`.
- **Unconfirmed:** the statement comes from a summarised rather than verbatim reading of the documentation. It must be checked against the documentation or an installation in phase 4 before it is implemented. Such statements are marked *(unconfirmed)* below.

## 2. How Claude Code controls file access

These documented ground rules govern the translation into findings:

1. **Working directory.** Claude Code may read files in the start directory and in additional directories without a prompt. Writing depends on the permission mode.
2. **Additional directories** (`permissions.additionalDirectories`, `--add-dir`, `/add-dir`) are treated like the working directory.
3. **Permission rules** (`allow`, `ask`, `deny`) apply to Claude's own tools. Order: `deny` first, then `ask`, then `allow`. A deny rule from any level beats every allow rule.
4. **Read/Edit deny rules** also apply to file commands recognised in Bash (`cat`, `head`, `sed`, redirections), but **not** to arbitrary subprocesses (e.g. a Python script) or commands that do not name the path (`grep -r pattern .`). Bash rules are text-based and, according to the documentation, "not a security boundary".
5. **The sandbox** only applies to shell commands (Bash, PowerShell, Monitor) and their child processes. File tools, MCP servers and hooks run outside the sandbox. Without a sandbox, a shell command can reach everything the user can reach.
6. **Sandbox defaults:** writing in the working directory, the per-user temp directory and additional directories; **reading the entire computer** except for a few denied areas. There is no sandbox on native Windows.
7. **MCP servers** are separate processes with the user's rights. Their file access follows from their program and arguments, not from Claude Code rules.

Consequences for the scanner:

- Configured paths and rules → findings with status `Configured`.
- Shell without sandbox → one summarising `Potential` finding "effectively user rights" (roadmap 4.4).
- Shell with sandbox → `Potential` findings for the areas the sandbox allows; reading stays computer-wide.
- Every MCP server with a local process → its own finding with subject `McpServer`.

## 3. Configuration locations

### 3.1 Settings files

| Level | Path | OS | Classification |
|---|---|---|---|
| Managed | `/Library/Application Support/ClaudeCode/managed-settings.json` | macOS | documented |
| Managed | `/etc/claude-code/managed-settings.json` | Linux, WSL | documented |
| Managed | `C:\Program Files\ClaudeCode\managed-settings.json` | Windows | documented |
| Managed (drop-ins) | `managed-settings.d/*.json` in the same directory, merged alphabetically after `managed-settings.json`; hidden files and other extensions are ignored | all | documented |
| User | `~/.claude/settings.json` or `$CLAUDE_CONFIG_DIR/settings.json` (Windows: `%USERPROFILE%\.claude\settings.json`) | all | documented |
| User, local | `~/.claude/settings.local.json`, only effective for sessions in the home directory | all | *(unconfirmed)* |
| Project | `<project>/.claude/settings.json` | all | documented |
| Project, local | `<project>/.claude/settings.local.json` (in Git repositories at the repository root, in worktrees in the main checkout) | all | documented |

Notes:

- Claude Code **no longer** reads the old Windows path `C:\ProgramData\ClaudeCode\managed-settings.json`. If the scanner finds a file there, it produces a diagnostic "deprecated location, not effective" but no findings.
- Managed files may be symlinks. If a link points outside the system directory, the file still applies; the scanner produces a warning.

### 3.2 Other managed sources

| Source | Location | Handling in the MVP |
|---|---|---|
| MDM, macOS | managed preferences domain `com.anthropic.claudecode` | detect and report as diagnostic "managed source present, not evaluated" |
| Registry, Windows | `HKLM\SOFTWARE\Policies\ClaudeCode`, value `Settings` | like MDM |
| Registry, Windows (user) | `HKCU\SOFTWARE\Policies\ClaudeCode`, value `Settings` | like MDM |
| Server-managed | admin console or gateway, loaded at startup | not visible locally; known limitation |
| Policy helper | key `policyHelper` (external program) | never executed; presence reported as diagnostic |
| SDK host | option `managedSettings` of an embedding app | not visible; known limitation |

Precedence of managed sources (documented): server-managed, then MDM/HKLM registry, then managed files, then HKCU. The default is `managedSourcesBehavior: "first-wins"`; with `"merge"` they are combined. Because the scanner cannot always see higher sources, findings from managed files carry a note that a higher, invisible source may override them.

### 3.3 `~/.claude.json`

A file Claude Code writes itself (documented). It contains account data, MCP servers of the user and local levels, per-project state (e.g. trust decisions) and global settings.

| Aspect | Decision | Classification |
|---|---|---|
| Location | `~/.claude.json` | documented |
| Location on Windows | `%USERPROFILE%\.claude.json` | heuristic |
| Location with `CLAUDE_CONFIG_DIR` | first `$CLAUDE_CONFIG_DIR/.claude.json`, then `~/.claude.json`; diagnostic states which one was read | heuristic |
| Areas read | only `mcpServers` and `projects` | – |
| Never read or output | `oauthAccount`, `userID`, `machineID`, caches and all other keys | – |

Evaluated keys under `projects["<absolute path>"]`:

| Key | Meaning | Classification |
|---|---|---|
| object key itself | known project path | heuristic (structure only documented in passing) |
| `mcpServers` | MCP servers of the local level | documented |
| `hasTrustDialogAccepted` | trust dialog accepted | documented |
| `enabledMcpjsonServers`, `disabledMcpjsonServers` | approval of individual `.mcp.json` servers | heuristic at this location (documented as settings keys) |
| `disabledMcpServers`, `enabledMcpServers` | servers toggled via `/mcp` | documented |
| `allowedTools` | older storage of allowed tools | heuristic; diagnostic only, if not empty |

Project keys observed locally but not evaluated: `mcpContextUris`, `hasClaudeMdExternalIncludesApproved`, `hasClaudeMdExternalIncludesWarningShown`. Unknown project keys produce no diagnostic, because the file contains many internal fields.

### 3.4 MCP configuration

| Level | Location | Classification |
|---|---|---|
| Managed | `managed-mcp.json` in the managed directory (see 3.1); same format as `.mcp.json`; exclusive if present | documented |
| Managed | key `managedMcpServers` in managed settings (`http`/`sse` only) | documented |
| Local | `~/.claude.json` → `projects["<path>"].mcpServers` | documented |
| Project | `<project>/.mcp.json` → `mcpServers` | documented |
| User | `~/.claude.json` → `mcpServers` (top level) | documented, key path only implied |
| Plugin | `.mcp.json` in the plugin directory or `mcpServers` in `plugin.json` | documented; **not in the MVP** |

Precedence for the same server name: local before project before user before plugin; `managedMcpServers` above all. `managed-mcp.json` excludes all other sources.

Server fields: `type` (`stdio` if neither `type` nor `url` is set; otherwise `http`, `streamable-http`, `sse`, `ws`), `command`, `args`, `env`, `url`, `headers`, `headersHelper`, `timeout`, `oauth`. Claude Code skips entries with `type: "sdk"`; so does the scanner.

Variables: `${VAR}` and `${VAR:-default}` in `command`, `args`, `env`, `url`, `headers`. The scanner expands only in `command` and `args` and only via the `ScanContext`; unset variables stay literal and produce a diagnostic.

### 3.5 Session data

| Location | Content | Classification |
|---|---|---|
| `~/.claude/projects/<encoded path>/` or `$CLAUDE_CONFIG_DIR/projects/…` | one directory per project with session logs (`<uuid>.jsonl`) and `memory/` | heuristic |
| `CLAUDE_CODE_PROJECT_DIR_NAME` | changes the subdirectory name under `projects/` | documented |

The directory name replaces path separators with `-`. The encoding is **not reversible** (`ai-agent-access-inspector` and `ai/agent/access/inspector` produce the same name). The scanner therefore never derives paths from the directory name but reads the `cwd` field from the most recent `.jsonl` file:

- read line by line, stop at the first record with `cwd`,
- keep only `cwd` and `version`, discard all other fields immediately,
- result: finding with status `Observed`, confidence `Low` to `Medium`.

## 4. Evaluated settings keys

### 4.1 Permissions

| Key | Meaning | Finding |
|---|---|---|
| `permissions.allow` | allowed tool calls | `Read(…)` / `Edit(…)` → Configured, effect Allow |
| `permissions.ask` | calls with prompt | `Read(…)` / `Edit(…)` → Configured, effect Ask |
| `permissions.deny` | forbidden calls | `Read(…)` / `Edit(…)` → Configured, effect Deny |
| `permissions.additionalDirectories` | additional working directories | Configured, effect Allow, access Read; Write depends on the mode |
| `permissions.defaultMode` | start mode: `default` (alias `manual`), `acceptEdits`, `plan`, `auto`, `dontAsk`, `bypassPermissions` | warning for `bypassPermissions` and `auto`; `acceptEdits` increases write access to working and additional directories |
| `permissions.disableBypassPermissionsMode` | `"disable"` locks bypass mode | defuses the bypass warning |
| `permissions.disableAutoMode` | `"disable"` locks auto mode | defuses the auto warning |
| `permissions.blockReadsOutsideWorkingDirectories` | `true`: file tools read nothing outside the working directories (from v2.1.257) | restricts read findings outside them; effective from any level |
| `allowManagedPermissionRulesOnly` | only managed rules apply | rules from other levels → finding with note "not effective" |

Other tool rules (`Bash(…)`, `WebFetch(…)`, `mcp__…`, `Agent(…)`) produce no folder findings. Exceptions:

- `Bash` or `Bash(*)` in `allow` → warning "shell allowed without prompt", strengthens the potential finding.
- `Bash`, `Read` or `Edit` without parentheses in `deny` → the tool is removed entirely; this restricts the corresponding potential finding.

Rules for `Write(…)`, `NotebookEdit(…)`, `Glob(…)` and `MultiEdit(…)` never match according to the documentation. The scanner reports them as diagnostic "ineffective rule".

### 4.2 Sandbox

| Key | Meaning | Finding |
|---|---|---|
| `sandbox.enabled` | sandbox for shell commands (default `false`) | determines whether the potential finding reads "user rights" or "sandbox area" |
| `sandbox.filesystem.allowWrite` | additional writable paths | Potential, effect Allow, access Write |
| `sandbox.filesystem.denyWrite` | blocked write paths | Configured, effect Deny, access Write |
| `sandbox.filesystem.denyRead` | blocked read paths (not for the Read tool) | Configured, effect Deny, access Read |
| `sandbox.filesystem.allowRead` | re-open reading inside `denyRead` | Configured, effect Allow, access Read |
| `sandbox.filesystem.disabled` | filesystem isolation off (user and managed level, `--settings` only) | sandbox counts as off for the filesystem; warning |
| `sandbox.excludedCommands` | commands outside the sandbox | warning per entry |
| `sandbox.allowUnsandboxedCommands` | retry without sandbox allowed (default `true`) | note on the sandbox finding |
| `sandbox.autoAllowBashIfSandboxed` | sandboxed commands without prompt (default `true`) | note on the sandbox finding |
| `sandbox.failIfUnavailable` | abort startup if no sandbox is possible | note |

The path syntax of sandbox keys differs from permission rules: `/` is absolute, `~/` relative to home, `./` or no prefix relative to the project (project settings) or to `~/.claude` (user settings) *(unconfirmed)*.

On native Windows the scanner reports `sandbox.enabled: true` with the warning "no effect on this system".

### 4.3 MCP approvals

| Key | Effect in the scanner |
|---|---|
| `enableAllProjectMcpServers` | all servers from `.mcp.json` count as approved |
| `enabledMcpjsonServers` | named `.mcp.json` servers count as approved |
| `disabledMcpjsonServers` | named servers count as rejected (from any level) |
| `allowedMcpServers`, `deniedMcpServers` | allow/deny lists with entries `{"serverName"}`, `{"serverCommand"}`, `{"serverUrl"}` |
| `allowManagedMcpServersOnly` | only the managed allowlist applies |

`enableAllProjectMcpServers` and `enabledMcpjsonServers` from `<project>/.claude/settings.json` only apply in trusted projects.

Approval state of a `.mcp.json` server: approved, rejected or **pending** (Claude Code asks at startup). Pending servers get status `Unknown`.

### 4.4 Other relevant keys

| Key | Handling |
|---|---|
| `hooks`, `disableAllHooks`, `allowManagedHooksOnly` | hooks run commands outside the sandbox → warning "hooks configured" with count; contents are not output |
| `env` | not read (may contain secrets); exception: `CLAUDE_CONFIG_DIR` from user and managed settings |
| `autoMemoryDirectory`, `plansDirectory` | not evaluated for now; check in phase 4 |

All other keys are ignored. Unknown keys inside `permissions` and `sandbox` produce a diagnostic.

## 5. Precedence and merging

Settings precedence (documented, highest first):

1. Managed
2. Command line (`--settings`) – invisible to the scanner
3. Project, local (`.claude/settings.local.json`)
4. Project (`.claude/settings.json`)
5. User (`~/.claude/settings.json`)

Rules for the scanner:

- **Lists are merged**, not replaced (`permissions.allow`, `deny`, `ask`, `additionalDirectories`, sandbox path lists). Each entry becomes its own finding with its source.
- **Single values** (`defaultMode`, `sandbox.enabled` etc.) are taken from the highest level that sets them; overridden values are recorded.
- **Deny rules** beat allow rules from any level; nothing can lift a managed deny.
- **Restrictions for project files:**
  - `permissions.allow` and `permissions.additionalDirectories` from `.claude/settings.json` only take effect after the trust dialog has been accepted. If `hasTrustDialogAccepted` in `~/.claude.json` is not `true`, the finding gets a warning "not yet effective".
  - `deny` and `ask` take effect immediately.
  - `defaultMode` with `auto` or `bypassPermissions` has no effect from project and local settings → diagnostic instead of warning.
  - `CLAUDE_CONFIG_DIR` in project or local settings is ignored.
- **Restrictive exception:** `permissions.blockReadsOutsideWorkingDirectories: true` applies from any level, even if managed settings set `false`.

## 6. Rule syntax and path resolution

Format: `Tool` or `Tool(specifier)`. `Read` and `Edit` use gitignore patterns.

| Prefix | Meaning | Example |
|---|---|---|
| `//path` | absolute from the filesystem root | `Read(//Users/alice/secrets/**)` |
| `~/path` | relative to the home directory | `Read(~/Documents/*.pdf)` |
| `/path` | relative to the **source of the setting** | see below |
| `path`, `./path` | relative to the current directory | `Read(*.env)` |

Anchor of `/path`:

| Source | Anchor |
|---|---|
| `.claude/settings.json`, `.claude/settings.local.json` | primary working directory (project) |
| `~/.claude/settings.json` | `~/.claude` |
| `--settings <file>` | directory of the file |

Important pitfalls, for which the scanner adds an explanation to the finding:

- `Read(/Users/alice/file)` is **not** absolute. In user settings it means `~/.claude/Users/alice/file`. The scanner detects this pattern (single slash followed by a typical root directory such as `Users`, `home`, `etc`) and warns "probably meant as an absolute path".
- Relative patterns in allow rules only apply from the working directory (`Edit(src/**)` = `<cwd>/src/**`), in deny/ask rules at any depth.
- `!pattern` in deny/ask rules is a negation that excludes paths from preceding relative rules.
- On Windows, paths are converted to POSIX form before matching; drives as `//c/…`.
- Symlinks: allow rules only apply if path **and** target match; deny rules if path **or** target matches.
- `.claudeignore` has no effect and is not evaluated.

Deriving the base path for findings: the longest prefix without glob characters (`*`, `?`, `[`, `{`, `!`). `Read(~/projects/**/*.md)` → base path `~/projects`, pattern kind `Glob`.

Access kind:

| Rule | `AccessKind` |
|---|---|
| `Read(…)` | Read |
| `Edit(…)` | Write (Edit rules cover all file-writing tools) |
| `additionalDirectories` | Read, in mode `acceptEdits`/`bypassPermissions` ReadWrite |
| `sandbox.filesystem.allowWrite` | Write |

## 7. Project discovery

A project is a directory whose project settings and `.mcp.json` are examined. Sources in this order, deduplicated:

1. `current_dir` from the `ScanContext`, if given
2. keys of `projects` in `~/.claude.json`
3. `cwd` from `~/.claude/projects/*/` (see 3.5)

For Git repositories the repository root is also checked for `.claude/settings.local.json`. Project directories that no longer exist produce a warning, not an abort.

## 8. Detecting the installation

Claude Code counts as **configured** if at least one of these exists: `~/.claude/`, `~/.claude.json`, `$CLAUDE_CONFIG_DIR`, a managed file.

The installation itself (program file) is not searched for in the MVP and `claude` is **never executed**. The version is read, if available, from the `version` field of the most recent session log (heuristic).

## 9. Privacy while reading

- From `~/.claude.json` only `mcpServers` and `projects` (see 3.3).
- From session logs only `cwd` and `version`, stopping after the first hit.
- MCP: `env`, `headers`, `headersHelper` and `oauth` are never output. In `args`, values that look like secrets are redacted (token prefixes, long random strings, values after `--token`, `--api-key` and similar).
- The settings key `env` is not read.
- Excerpts contain only the evaluated value, never the surrounding file.

## 10. MCP servers as findings

For every effective MCP server:

| Server type | Finding |
|---|---|
| `stdio` | subject `McpServer`, status Potential, access Unknown, explanation "local process with user rights" |
| `stdio` with path-like arguments | additionally one finding per path, status Configured, confidence Medium |
| known filesystem server (`@modelcontextprotocol/server-filesystem`) | path arguments with access ReadWrite, confidence High |
| `http`, `sse`, `ws` | no folder finding; the server is listed in the overview |

An argument is path-like if it starts with `/`, `~`, `./`, `../`, a drive letter or `\\` and is not a flag. The list of known servers is defined in phase 4 and kept deliberately short.

## 11. Known limitations

These points are documented visibly in the application and the README:

- **Session-only settings** (`--add-dir`, `/add-dir`, `--settings`, `--allowedTools`, `--permission-mode`) are not stored and invisible to the scanner.
- **Server-managed settings**, policy helpers and SDK host settings are not visible locally.
- **MDM profiles and registry values** are only detected in the MVP, not evaluated.
- **Bash rules** are not a security boundary; actual reachability depends on the sandbox.
- **Read deny rules** do not prevent access by arbitrary subprocesses.
- **MCP servers** can access everything the user can reach, regardless of their arguments.
- **Plugins** (including plugin MCP servers) are not evaluated in the MVP.
- **Heuristic sources** (`~/.claude.json` project list, session logs) may change without notice.

## 12. Fixture requirements

Phase 4 needs at least the following under `fixtures/claude-code/`:

| Fixture | Content |
|---|---|
| `macos/basic` | user settings, one project with project and local settings, `~/.claude.json` with one project |
| `linux/basic` | like macOS, Linux paths, managed file under `/etc/claude-code` |
| `windows/basic` | Windows paths, drive and `//c/` patterns, managed path under `Program Files`, old `ProgramData` path |
| `managed/drop-ins` | `managed-settings.json` plus `managed-settings.d/` with several files and one hidden file |
| `rules/anchors` | all four prefixes, the `/Users/…` pitfall, negation, glob base paths |
| `rules/conflicts` | allow and deny on the same path from different levels |
| `modes/bypass` | `defaultMode: bypassPermissions` in user and project settings, with and without `disableBypassPermissionsMode` |
| `sandbox/on` and `sandbox/off` | sandbox with `allowWrite`, `denyRead`, `excludedCommands`; without sandbox |
| `mcp/scopes` | servers on all levels, name conflict, filesystem server, server with an `env` secret |
| `mcp/approval` | `.mcp.json` with approved, rejected and pending servers |
| `trust/untrusted` | project with `allow` rules without accepted trust dialog |
| `sessions/encoded` | `projects/` directory with an ambiguous name and `cwd` in the `.jsonl` |
| `config-dir` | `CLAUDE_CONFIG_DIR` set, `.claude.json` at both possible locations |
| `malformed/*` | invalid JSON, truncated file, wrong types, unknown keys, unreadable file |

Fixtures contain only invented paths and values.

## 13. Consequences for the data model (phase 2)

- `FindingSubject` (agent / MCP server) and `RuleEffect` (allow / ask / deny) are confirmed.
- Additionally needed: a **"not effective"** marker with a reason (trust dialog missing, only managed rules allowed, level may not set the key, ineffective rule). Instead of a new status this is modelled as a warning of its own kind.
- `FindingSource` needs the **anchor** for relative patterns (project, `~/.claude`, file directory).
- `AccessKind` is sufficient; `additionalDirectories` additionally depends on the mode, which is recorded in the explanation.
- A **scan summary** per agent (mode, sandbox on/off, number of MCP servers, unevaluated managed sources) complements the list of findings.

## 14. Open checks for phase 4

To be verified against documentation or an installation before implementation:

- `~/.claude/settings.local.json` and its validity *(unconfirmed)*
- path syntax of sandbox keys *(unconfirmed)*
- location of `.claude.json` when `CLAUDE_CONFIG_DIR` is set
- exact types of `enabledMcpjsonServers` / `disabledMcpjsonServers` (presumably a list of server names)
- complete list of keys that only work in managed settings
