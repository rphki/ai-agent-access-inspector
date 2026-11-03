# Specification: ChatGPT/Codex scanner

Result of phase 8.1 of the [roadmap](roadmap.md). This document defines which files the ChatGPT/Codex scanner reads, which keys it evaluates and how it translates them into findings.

## 1. Status and scope

| | |
|---|---|
| Research date | 2026-11-03 |
| Documentation checked | learn.chatgpt.com/docs (Codex), as of 2026-11-03 |
| Local installation examined | ChatGPT desktop app 26.1021 (macOS, bundle ID `com.openai.codex`) with bundled Codex CLI 0.164.0-alpha |
| Supported format | only the currently documented one (see roadmap, section 2) |
| Scanner ID and display name | `codex`, "ChatGPT / Codex" |
| Implementation | planned in phase 8.3 under `crates/inspector-core/src/scanners/codex/` |

The ChatGPT desktop app has merged with the Codex app: it is the same application (bundle ID `com.openai.codex`), runs the bundled Codex CLI as its agent and shares `~/.codex` with a separately installed Codex CLI and the IDE extension. One scanner covers all of them (roadmap, section 2).

Out of scope: the chat side of ChatGPT (connectors, file uploads, remote MCP servers in the ChatGPT account). It is configured server-side and is not visible locally.

Sources (official documentation; the former `developers.openai.com/codex/*` pages redirect there):

- [Config basics](https://learn.chatgpt.com/docs/config-file/config-basic)
- [Advanced config](https://learn.chatgpt.com/docs/config-file/config-advanced)
- [Config reference](https://learn.chatgpt.com/docs/config-file/config-reference)
- [Environment variables](https://learn.chatgpt.com/docs/config-file/environment-variables)
- [Approvals and security](https://learn.chatgpt.com/docs/agent-approvals-security)
- [Sandboxing](https://learn.chatgpt.com/docs/sandboxing)
- [Permissions](https://learn.chatgpt.com/docs/permissions)
- [Permission modes (app)](https://learn.chatgpt.com/docs/permission-modes)
- [Managed configuration](https://learn.chatgpt.com/docs/enterprise/managed-configuration)
- [Windows sandbox](https://learn.chatgpt.com/docs/windows/windows-sandbox)
- [MCP](https://learn.chatgpt.com/docs/extend/mcp)
- [CLI commands](https://learn.chatgpt.com/docs/developer-commands?surface=cli)

### Source classification

Same as for Claude Code ([specification](claude-code-scanner.md), section 1): **documented**, **heuristic** (at most `Confidence::Medium`), **unconfirmed** (the statement could not be confirmed verbatim; section 14 records how it is handled).

## 2. How Codex controls file access

1. **Sandbox.** Every command Codex runs goes through an OS sandbox (Seatbelt on macOS, bubblewrap on Linux/WSL2, a Windows sandbox). Three modes: `read-only`, `workspace-write`, `danger-full-access`.
2. **Workspace.** In `workspace-write`, Codex may write in the working directory, the configured `writable_roots`, directories added with `--add-dir`, `$TMPDIR` and `/tmp` (the last two can be excluded).
3. **Protected paths.** Inside a writable root, `.git` (directory or file) and the directories `.agents`, `.codex` and `.aws` stay read-only, recursively.
4. **Reading.** `read-only` "can inspect files"; how far reading reaches is not documented (unconfirmed, see 14.1). The previous Seatbelt base policy allowed reading the whole file system.
5. **Permission profiles** (newer) replace the sandbox mode: `default_permissions` selects a profile, and `[permissions.<name>]` defines `read`, `write` and `deny` per path or glob. They are ignored as soon as `sandbox_mode` is set anywhere.
6. **Approval policy** decides when Codex asks before leaving the sandbox (`on-request`, `never`, granular).
7. **Trust.** Project configuration is only loaded for trusted projects. Version-controlled folders get Auto (`workspace-write` + `on-request`), others start `read-only`.
8. **MCP servers** are separate processes. Whether they run inside the sandbox is not documented (unconfirmed, see 14.2).

Consequences for the scanner:

- Configured roots, rules and trusted projects → findings with status `Configured`.
- Effective sandbox mode → `Potential` findings for the areas it opens (temp directories, reading, everything for full access).
- Every local MCP server → its own finding with subject `McpServer`, as for Claude Code.
- Enabled broad-access plugins → one heuristic hint each (section 10).

## 3. Configuration locations

### 3.1 Codex home

`$CODEX_HOME`, default `~/.codex` (Windows: `%USERPROFILE%\.codex`). The scanner takes `CODEX_HOME` from the scan environment via `ScanContext`. Documented.

### 3.2 Configuration files

| Level | Path | Scope | Classification |
|---|---|---|---|
| User | `$CODEX_HOME/config.toml` | `User` | documented |
| Profile | `$CODEX_HOME/<name>.config.toml` | `User` | documented, only selected per session with `--profile` (section 5) |
| Project | `<dir>/.codex/config.toml` from the project root down to the working directory, closest wins; only for trusted projects | `Project` | documented |
| System | `/etc/codex/config.toml` (Unix only) | `Managed` | documented |
| Managed (legacy) | `/etc/codex/managed_config.toml` (macOS, Linux), `$CODEX_HOME/managed_config.toml` (Windows) | `Managed` | documented |
| Requirements | `/etc/codex/requirements.toml` (macOS, Linux), `%ProgramData%\OpenAI\Codex\requirements.toml` (Windows) | `Managed` | documented |

The project root is the nearest directory with `.git` (`project_root_markers` can change this; `[]` makes the working directory the root). Project configuration ignores some keys (`notify`, `profile(s)`, providers, base URLs); none of them affects file access.

### 3.3 Other managed sources

- **macOS MDM:** preference domain `com.openai.codex`, keys `config_toml_base64` and `requirements_toml_base64`. Like Claude Code, only **detected** (file under `/Library/Managed Preferences/`), not evaluated (roadmap, section 21).
- **Cloud-managed configuration and requirements** for the signed-in workspace: not visible locally.
- **Windows policies:** not documented.

### 3.4 App state `.codex-global-state.json`

`$CODEX_HOME/.codex-global-state.json`, written by the desktop app. Undocumented, **heuristic**, about 350 KiB locally. Read only:

| Key | Content | Use |
|---|---|---|
| `local-projects.<id>.rootPaths` | folders of projects added in the app | working directories (section 7) |
| `thread-workspace-root-hints.<thread>` | workspace folder of a chat without a project | observed working directories |
| `thread-projectless-output-directories.<thread>` | output folder of a chat without a project | observed write targets |

Everything else in the file (window state, model choices, thread read states, `electron-persisted-atom-state`) is not read. Not evaluated, but noted: `electron-persisted-atom-state.agent-mode-by-host-id` holds the permission mode selected in the app (observed: `auto`), and `heartbeat-thread-permissions-by-id` holds the sandbox policy of individual automations.

### 3.5 Files that are never read

`auth.json`, `sessions/`, `archived_sessions/`, `session_index.jsonl`, all `*.sqlite` files, `logs*`, `transcription-history.jsonl`, `dictation-history/`, `memories*`, `.sandbox/`, `.sandbox-secrets/`, `shell_snapshots/`, `browser/`, `computer-use/`, `plugins/cache/`, `.chatgpt-projects/`.

## 4. Evaluated keys

### 4.1 Sandbox

| Key | Values | Use |
|---|---|---|
| `sandbox_mode` | `read-only`, `workspace-write`, `danger-full-access` | effective mode (section 5) |
| `sandbox_workspace_write.writable_roots` | array of paths | one finding each, `Write`, `Allow` |
| `sandbox_workspace_write.exclude_tmpdir_env_var` | bool, default `false` | `$TMPDIR` finding on/off |
| `sandbox_workspace_write.exclude_slash_tmp` | bool, default `false` | `/tmp` finding on/off |
| `sandbox_workspace_write.network_access` | bool, default `false` | summary only |
| `windows.sandbox` | `mxc`, `elevated`, `unelevated` | summary; `unelevated` does not enforce deny-read (warning) |

### 4.2 Permission profiles

Evaluated only if no loaded file sets `sandbox_mode`.

| Key | Use |
|---|---|
| `default_permissions` | active profile; built-ins `:read-only`, `:workspace`, `:danger-full-access` |
| `permissions.<name>.extends` | inherited profile (chain resolved, cycles become a diagnostic) |
| `permissions.<name>.workspace_roots."<path>" = true` | working directory finding |
| `permissions.<name>.filesystem."<path>" = "read" \| "write" \| "deny"` | one finding each, effect `Allow` (read, write) or `Deny` |
| `permissions.<name>.filesystem.":workspace_roots"."<subpath>"` | one finding per workspace root and subpath |

Path tokens: `:root` (only with subpath `.`, means the whole file system), `:minimal`, `:workspace_roots`, `:tmpdir`, `:slash_tmp`, absolute paths, `~/…`, globs (`**/*.env`). Precedence: `deny` beats `write` beats `read`; a more specific entry beats a broader one. `:minimal` (system files Codex needs to run) is shown as one `Potential` read finding without a path list.

### 4.3 Approval policy

| Key | Values | Use |
|---|---|---|
| `approval_policy` | `on-request`, `never`, `{ granular = { … } }`; `untrusted` retired, `on-failure` deprecated | summary; `never` combined with full access gets a warning |
| `approvals_reviewer` | `user`, `auto_review` | summary |

Inside the sandbox Codex acts without asking, so sandbox findings get effect `Allow`. Leaving the sandbox needs approval with `on-request`; this is mentioned in the explanation, not modelled as separate `Ask` findings.

### 4.4 Projects

| Key | Use |
|---|---|
| `projects."<path>".trust_level` | `trusted` → working directory finding and project configuration is loaded; `untrusted` → project configuration is skipped (diagnostic if one exists) |

### 4.5 Requirements (`requirements.toml`)

| Key | Use |
|---|---|
| `allowed_sandbox_modes`, `allowed_permission_profiles`, `allowed_approval_policies` | a configured value that is not allowed is marked "not effective" (warning) and the effective mode falls back to the most restrictive allowed one |
| `default_permissions` | managed default profile |
| `permissions.filesystem.deny_read` | one `Deny` finding each, scope `Managed`; full-access profiles become not effective |
| `mcp_servers.<name>` | allow-list: servers not listed are not effective; an empty table disables all |
| `features.computer_use`, `features.browser_use` | `false` disables the corresponding plugin hint |

Other requirement keys (network, rules, hooks, web search, remote sandbox config) are not evaluated.

### 4.6 MCP servers

`[mcp_servers.<id>]` in user and trusted project configuration. Read: `command`, `args`, `cwd`, `url`, `enabled`. Never read or output: `env`, `env_vars`, `http_headers`, `env_http_headers`, `http_headers_helper`, `bearer_token_env_var`, `auth`, `oauth`.

### 4.7 Plugins

`[plugins."<name>@<marketplace>"].enabled` and `plugins._default.enabled`. Plugin-bundled MCP servers and skills are not evaluated (roadmap, section 21).

## 5. Precedence and merging

Configuration, highest first:

1. CLI flags and `-c` overrides, `--sandbox`, `--add-dir`, `--yolo`: session-only, invisible
2. trusted project configuration (closest first)
3. profile file selected with `--profile`: session-only, so profile files are listed in the summary but not merged
4. user configuration
5. cloud-managed defaults: invisible
6. `/etc/codex/config.toml`
7. built-in defaults

Managed layers override all of these: MDM `config_toml_base64` (detected only), then `managed_config.toml`. Requirements constrain the result last and cannot be relaxed (order: system `requirements.toml`, cloud, legacy managed fields, MDM).

Tables are merged key by key; arrays are replaced, not concatenated (unconfirmed, see 14.3).

**Default without a configured mode:** a trusted project with version control gets `workspace-write` + `on-request`; anything else `read-only`. The scanner applies the default per project and states it in the explanation (classification `derived`).

## 6. Path resolution

- Paths in `config.toml` are absolute or start with `~`; relative paths in `writable_roots` and profile entries are resolved against the directory of the configuration file and get a warning (unconfirmed, see 14.4).
- Project keys in `[projects."<path>"]` are absolute target paths.
- `$TMPDIR` comes from the scan environment; if it is not set, the finding has no path and says so.
- Windows paths follow the existing `TargetPath` rules.

## 7. Project discovery

Working directories come from:

1. `projects.*` with `trust_level = "trusted"` (documented, confidence High)
2. `permissions.<active>.workspace_roots` (documented, High)
3. `local-projects.*.rootPaths` in the app state (heuristic, Medium)
4. `thread-workspace-root-hints` (heuristic, status `Observed`, Low)

Duplicates are merged by the existing deduplication. For every working directory the scanner looks for `.codex/config.toml` from the project root down to that directory, but only if the project is trusted.

## 8. Detecting the installation

Codex counts as **configured** if at least one exists: `$CODEX_HOME/`, `/etc/codex/`, the Windows requirements file, the MDM preference file.

The app is recognised by `/Applications/ChatGPT.app/Contents/Resources/codex-cli/codex-package.json` (macOS, heuristic); its `version` field is the reported version. A separately installed CLI is not searched for, and `codex` is **never executed**.

## 9. Privacy while reading

- Only the files in 3.2 and 3.4, only the keys in section 4 and 3.4.
- TOML and JSON files only if they are regular files no larger than 16 MiB (existing limit).
- MCP secrets as in 4.6; `command`, `args` and `url` are redacted like for Claude Code.
- The keys `notify`, `model*`, `marketplaces`, `skills`, `desktop`, `shell_environment_policy` are not read.
- Excerpts contain only the evaluated value.

## 10. Findings

| Source | Status | Access | Effect | Confidence |
|---|---|---|---|---|
| working directory (section 7) in `workspace-write` | Configured | ReadWrite | Allow | per section 7 |
| working directory in `read-only` | Configured | Read | Allow | per section 7 |
| protected subpaths `.git`, `.agents`, `.codex`, `.aws` of a writable root | Configured | Write | Deny | High, `derived` |
| `writable_roots` entry | Configured | Write | Allow | High |
| profile `filesystem` entry | Configured | Read / Write | Allow / Deny | High |
| `deny_read` | Configured, scope Managed | Read | Deny | High |
| `$TMPDIR`, `/tmp` in `workspace-write` | Potential | Write | Allow | High |
| reading in `read-only` / `workspace-write` | Potential, pattern `Everything` | Read | Allow | Medium (unconfirmed, 14.1) |
| `danger-full-access` or `:danger-full-access` | Potential, pattern `Everything` | ReadWrite | Allow | High, warning |
| `thread-projectless-output-directories` | Observed | Write | Allow | Low |
| local MCP server | as for Claude Code ([specification](claude-code-scanner.md), section 10) | | | |
| enabled plugin `computer-use`, `unified-computer-use`, `browser` | Potential, no path | Unknown | NotApplicable | Low, heuristic |

The plugin hint explains that the plugin lets Codex operate other applications or the browser with the user's rights, so file access is possible beyond the sandbox. Other enabled plugins appear only in the summary.

**Scan summary:** effective sandbox mode or permission profile, approval policy, network access, Windows sandbox, listed profile files, number of MCP servers, enabled plugins, detected but unevaluated managed sources (MDM, cloud), app or CLI.

## 11. Known limitations

- **Session-only settings** (`--sandbox`, `--add-dir`, `--profile`, `-c`, `--yolo`, the permission mode chosen per chat in the app) are invisible.
- **Cloud-managed** configuration and requirements are not visible locally; **MDM** values are only detected.
- **Reading scope** of the sandbox is not documented (14.1).
- **MCP servers** may run outside the sandbox and reach everything the user can reach.
- **Plugins** are only hinted at; their own permissions (e.g. `computer_use.*`, `browser_use.*`) are not evaluated.
- **App state** (`.codex-global-state.json`) is undocumented and may change without notice.
- **Chat side of ChatGPT** (connectors, uploads) is out of scope.

## 12. Fixture requirements

Phase 8.4 needs at least the following under `fixtures/codex/`:

| Fixture | Content |
|---|---|
| `macos/basic` | user config with trusted project, app state with `local-projects`, `codex-package.json` in an app bundle |
| `linux/basic` | Linux paths, `/etc/codex/config.toml`, `requirements.toml` |
| `windows/basic` | Windows paths, `%ProgramData%` requirements, `managed_config.toml` in Codex home, `windows.sandbox = "unelevated"` with `deny_read` |
| `sandbox/workspace-write` | `writable_roots`, both tmp excludes |
| `sandbox/full-access` | `danger-full-access` with `approval_policy = "never"` |
| `profiles/custom` | `default_permissions` with `extends`, `filesystem` read/write/deny, globs, `:workspace_roots` subpath, a cycle |
| `profiles/ignored` | `default_permissions` together with `sandbox_mode` |
| `projects/untrusted` | project with `.codex/config.toml` but `trust_level = "untrusted"` |
| `requirements/constrained` | disallowed sandbox mode, MCP allow-list, `features.computer_use = false` |
| `mcp/servers` | stdio filesystem server, HTTP server, server with `env` and `http_headers` secrets |
| `plugins/broad` | computer use, browser and an unrelated plugin enabled |
| `codex-home` | `CODEX_HOME` set to a non-default folder |
| `malformed/*` | invalid TOML, wrong types, unknown keys, invalid app state JSON, unreadable file |

Fixtures contain only invented paths and values.

## 13. Consequences for the implementation (phase 8.2)

- **TOML parser** in `inspector-core` (`toml` crate, parse into a value tree; `cargo deny`).
- **No model change for source positions:** `FindingSource.pointer` uses RFC 6901 syntax over the parsed TOML tables just as over JSON (e.g. `/sandbox_workspace_write/writable_roots/0`); `docs/findings-model.md` states this.
- `FindingScope`, `FindingStatus`, `AccessKind` and `RuleEffect` suffice.
- New message keys for the explanations, warnings, diagnostics and summary labels of sections 4 to 10.

## 14. Open checks (for phase 8.3)

1. **Reading scope** of `read-only` and `workspace-write`: check the sandbox policies in the `openai/codex` source (Seatbelt policy, `:read-only` profile). Until confirmed: `Potential` read of everything with confidence Medium.
2. **MCP servers and the sandbox:** check in the source whether stdio servers are started sandboxed. Until confirmed: treated like Claude Code (user rights).
3. **Array merging** across configuration layers: check in the source.
4. **Relative paths** in `writable_roots` and profile entries: check whether they are allowed and what they are relative to.
5. **Default of `windows.sandbox`** and whether `--full-auto` still exists: check in the source; only affects the summary.
