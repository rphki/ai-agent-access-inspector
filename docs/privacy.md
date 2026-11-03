# Privacy

The AI Agent Access Inspector works locally and only reads. This page describes which data it touches, what it shows and what leaves the computer: nothing, unless you export a report yourself.

## 1. Binding properties

- No network access, no telemetry, no automatic error reports, no updater.
- No change to any agent configuration; files are only opened for reading.
- No program is executed: neither the agent nor configured MCP servers or helper programs.
- Exports are only written on an explicit action, to the file you choose.

## 2. What is read

Only the configuration files of the supported agents, at the locations listed in the specifications of the [Claude Code scanner](claude-code-scanner.md) and the [ChatGPT/Codex scanner](codex-scanner.md). The scanners read only the keys they evaluate and drop everything else right after parsing; for example, account data in `~/.claude.json` is never kept. Session logs are read only up to their first lines, to find the project directory. Files are only read if they are regular files below a size limit.

For ChatGPT/Codex, the scanner reads `config.toml` files, `requirements.toml`, and from the app's state file `~/.codex/.codex-global-state.json` only the project folders, the folders of chats without a project and the permission mode selected in the app. It never reads `auth.json`, sessions, logs, histories or the app's databases.

Whether a path exists, and whether it is a file, directory or symlink, is checked on disk. File contents in these locations are not read.

## 3. What is never shown

- Environment variables, headers and OAuth settings of MCP servers (Claude Code: `env`, `headers`, `headersHelper`, `oauth`; Codex: `env`, `env_vars`, `http_headers`, `env_http_headers`, `bearer_token_env_var`, `auth`, `oauth`), and the URLs of remote servers.
- Values that look like secrets, wherever they appear in the output. They are replaced by `<redacted>` (section 4).
- Complete configuration files.

## 4. Redaction

Before a scan result is shown or exported, all free text in it passes through the redaction in `inspector-core` (`redact.rs`): rule patterns, values from configuration files, MCP command lines and message parameters. The following are replaced:

- values of secret flags such as `--token`, `--api-key`, `--password` (`--token abc` and `--token=abc`),
- values of secret-looking assignments such as `GITHUB_TOKEN=…`, `DB_PASSWORD=…`,
- authentication headers and schemes: `Authorization: Bearer …`, `X-Api-Key: …`,
- known token formats (`ghp_`, `github_pat_`, `glpat-`, `sk-`, `xoxb-`, `AKIA`, `AIza`, …),
- URLs with secret query parameters (`?token=`, `?key=`, …),
- long random-looking strings without a path separator.

Redaction is a **heuristic**. A secret without a recognisable format that is given as a plain argument (e.g. `server mypassword`) is not recognised. Paths are never redacted. If you share a report, read it first.

Finding ids are computed from the redacted values, so they do not carry a trace of a secret either.

## 5. Exports and path privacy

Reports can be exported as JSON (machine-readable, English texts with message keys) or Markdown (readable; from the GUI in its language, from the CLI in English). Before exporting you choose how paths are shown:

| Option | GUI | CLI | Effect |
|---|---|---|---|
| Full paths | "Full paths" | `--paths full` (default) | paths as they are |
| Home as `~` | "Home directory as ~" | `--paths home` | the own home directory becomes `~`, e.g. `/Users/alice/app` → `~/app` |
| Anonymised | "… and anonymised user names" | `--paths anonymized` | additionally, names in user directories (`/Users/<name>`, `/home/<name>`, `C:\Users\<name>`, also in Claude Code's encoded project directories such as `-home-alice-app`) become `user`; other users become `user2`, `user3`, … |

With `home` and `anonymized`, finding ids are renumbered (`finding-1`, …), because regular ids are derived from paths and could be used to confirm a guessed path.

Limits: anonymisation replaces the home directory and names in user directories. Other personal names in paths (e.g. `/Volumes/Alice's Disk`, project folders named after people) and host names are kept. In the anonymised mode, a word in a dash-separated name that follows `-home-` or `-Users-` (e.g. `my-home-made-tool`) is replaced too; that hides more than necessary but never less.

## 6. Where data stays

- The GUI keeps the last scan in memory only; it is gone when the application is closed. The chosen language is remembered in the web view's local storage.
- Nothing is written anywhere except export files you save yourself.

See [SECURITY.md](../SECURITY.md) for reporting vulnerabilities and [security-checks.md](security-checks.md) for how these properties are checked in each phase.
