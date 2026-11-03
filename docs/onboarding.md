# Onboarding: building, testing and CI

This guide is for anyone who checks out the repository and wants to understand how it is built, tested and checked. For what the tool does and why, start with the [README](../README.md) and the [roadmap](roadmap.md); for how the code is structured, see [architecture](architecture.md).

## 1. Prerequisites

| Tool | Needed for | Install |
|---|---|---|
| [rustup](https://rustup.rs/) | Rust toolchain | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` (macOS/Linux) or `rustup-init.exe` (Windows) |
| Git | version control | system package manager |
| [GitHub CLI](https://cli.github.com/) (`gh`) | optional: CI results in the terminal | e.g. `brew install gh` |
| [cargo-deny](https://embarkstudios.github.io/cargo-deny/) | optional: dependency audit before pushing | `brew install cargo-deny` or `cargo install --locked cargo-deny` |
| [Node.js](https://nodejs.org/) 24 and [pnpm](https://pnpm.io/) | GUI frontend (`src/`) | e.g. `brew install node pnpm`; the pnpm version is pinned in `package.json` (`packageManager`) |
| WebKitGTK and friends (Linux only) | building the GUI crate | `sudo apt-get install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev` (see [Tauri prerequisites](https://tauri.app/start/prerequisites/)) |

You do **not** need to pick a Rust version. The repository pins it (see 3.1); rustup installs the right toolchain on the first `cargo` call inside the repository. If your rustup version does not do that automatically, run `rustup show` once in the repository.

macOS needs the Xcode command line tools, Windows the WebView2 runtime (preinstalled on Windows 11).

## 2. First steps

```bash
git clone https://github.com/rphki/ai-agent-access-inspector.git
cd ai-agent-access-inspector
cargo build
cargo test
cargo run -p inspector-cli -- scan --agent demo
pnpm install
pnpm tauri dev
```

`cargo run` prints a JSON report with example findings from the demo scanner. `pnpm tauri dev` starts the Vite dev server and opens the GUI, which scans your real configuration (read-only). If all commands succeed, your setup is complete.

## 3. How the build is set up

### 3.1 Toolchain: `rust-toolchain.toml`

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

rustup reads this file whenever you run `cargo` in the repository. It selects the current stable Rust and makes sure the formatter (`rustfmt`) and the linter (`clippy`) are installed. CI uses the same file, so local builds and CI builds use the same toolchain configuration.

`rust-version` in `Cargo.toml` declares the oldest Rust version the code is meant to compile with. It is not verified in CI; CI always uses the current stable release. It also caps the dependency versions `cargo update` picks, so it must not be lower than security fixes require (raised to 1.88 for this reason in phase 5).

### 3.2 Workspace: `Cargo.toml`

The repository is a Cargo **workspace**: one root `Cargo.toml` that ties several crates (packages) together. They share one `Cargo.lock`, one `target/` directory and common settings.

| Crate | Path | Kind | Produces |
|---|---|---|---|
| `inspector-core` | `crates/inspector-core` | library | data model, scan context, scanners, path analysis |
| `inspector-cli` | `crates/inspector-cli` | binary | the program `ai-agent-access-inspector` |
| `inspector-gui` | `src-tauri` | binary and library | the desktop application (Tauri); its frontend lives in `src/` |

The root `Cargo.toml` defines for all crates:

- **`[workspace.package]`**: version, Rust edition (2024), license and repository URL. Each crate inherits them with `version.workspace = true` and so on, so the version is changed in one place.
- **`[workspace.dependencies]`**: the internal dependency on `inspector-core`.
- **`[workspace.lints]`**: compiler and Clippy rules (see 3.5).

`inspector-cli` and `inspector-gui` depend on `inspector-core`; the core has no dependency on either of them or on Tauri.

### 3.3 Dependencies: `Cargo.lock`

`Cargo.toml` states which versions of external crates are acceptable (e.g. `serde = "1.0.229"` means "1.0.229 or any newer 1.x"). `Cargo.lock` records the exact versions that were actually resolved. It is committed, so everyone and CI build the same versions.

- `cargo add <crate>` / `cargo remove <crate>` change `Cargo.toml` and update `Cargo.lock`.
- `cargo update` moves to newer compatible versions; commit the changed `Cargo.lock`.
- `--locked` (used in CI) makes Cargo fail instead of silently updating `Cargo.lock` if it does not match `Cargo.toml`. If CI fails with a message about the lock file, run `cargo build` locally and commit the updated `Cargo.lock`.

Current external dependencies: `serde` and `serde_json` (core), `clap` and `serde_json` (CLI), `tauri`, `tauri-plugin-dialog`, `tauri-plugin-opener`, `serde` and `serde_json` (GUI). New dependencies in the core are added sparingly.

### 3.4 Dependency audit: `deny.toml`

`cargo deny check` checks all dependencies against `deny.toml`:

- **advisories:** known vulnerabilities and yanked crates from the [RustSec](https://rustsec.org/) database; the few ignored advisories (unmaintained crates in Tauri's Linux tree without a safe upgrade) are listed with a reason,
- **licenses:** only the permissive licenses listed in `deny.toml` and MPL-2.0 (required by Tauri, see the comments there) are allowed; a dependency with another license fails the check and has to be discussed before it is added,
- **bans:** duplicate versions of a crate produce a warning, wildcard version requirements an error,
- **sources:** crates may only come from crates.io.

Run it whenever you add or update a dependency. CI runs it on every push and weekly (see 6.2).

### 3.5 Lints and formatting

Defined once in the root `Cargo.toml` and enabled in every crate with `[lints] workspace = true`:

- `unsafe_code = "forbid"`: `unsafe` Rust is not allowed anywhere.
- `clippy::all = "warn"`: all of Clippy's default lints.

Locally these are warnings. CI runs Clippy with `-D warnings`, which turns every warning into an error, so **code with warnings does not pass CI**.

Formatting follows the default `rustfmt` style (no `rustfmt.toml`). `cargo fmt --all` formats everything; CI checks it with `--check`.

`.gitattributes` forces LF line endings for all text files, so formatting checks and fixture files behave the same on Windows as on macOS and Linux.

### 3.6 Build output

Everything Cargo produces goes to `target/` (ignored by Git):

| Command | Output |
|---|---|
| `cargo build` | `target/debug/ai-agent-access-inspector` (unoptimised, with debug info) |
| `cargo build --release` | `target/release/ai-agent-access-inspector` (optimised) |
| `cargo run -p inspector-cli -- <args>` | builds and runs the debug binary; arguments go after `--` |

On Windows the binary has the suffix `.exe`.

### 3.7 Frontend: `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`

The GUI frontend (`src/`) is React with TypeScript, built by Vite into `dist/`, which `src-tauri/tauri.conf.json` embeds. `package.json` pins exact versions; `pnpm-lock.yaml` pins the whole tree and is used with `--frozen-lockfile` in CI. `pnpm-workspace.yaml` only holds pnpm settings, currently an `overrides` entry that lifts a vulnerable transitive dev dependency. pnpm refuses packages younger than its minimum release age; pick an older version rather than adding an exclusion.

| Command | Does |
|---|---|
| `pnpm dev` | Vite dev server on port 1420 (used by `pnpm tauri dev`) |
| `pnpm build` | type check and production build into `dist/` |
| `pnpm typecheck`, `pnpm lint`, `pnpm test` | TypeScript, ESLint, Vitest |
| `pnpm tauri dev` | GUI with hot reload of the frontend; Rust changes rebuild the app |

The GUI translates core messages by key. `src/locales/en.json` is generated from the core catalog (`model/message.rs`); the test `gui_catalog_is_in_sync` fails when they differ. After adding a catalog entry, run `UPDATE_EXPECT=1 cargo test -p inspector-core gui_catalog` and add the German text to `src/locales/de.json` (a frontend test checks that German covers every key). GUI-only texts live in `src/locales/ui.en.json` and `ui.de.json`.

## 4. Tests

```bash
cargo test --workspace                       # everything
cargo test -p inspector-core                 # only the core
cargo test -p inspector-core analysis::      # only tests whose name contains "analysis::"
cargo test -p inspector-cli --test cli       # only the CLI integration tests
cargo test -p inspector-gui                  # GUI commands (path validation)
pnpm test                                    # frontend tests (Vitest)
```

| Kind | Where | What it does |
|---|---|---|
| Unit tests | `#[cfg(test)] mod tests` at the end of a source file, or a `tests.rs` next to it (e.g. `analysis/tests.rs`) | test functions of one module, with access to private items |
| Integration tests | `crates/inspector-cli/tests/cli.rs` | start the built binary as a separate process and check its output and exit code |
| Fixture tests | `crates/inspector-core/tests/claude_code_fixtures.rs`, `codex_fixtures.rs` (helpers in `tests/common/mod.rs`) | scan every fixture in `fixtures/claude-code/` or `fixtures/codex/` and compare the result with its `expected.json` |
| Frontend tests | `src/**/*.test.ts(x)` | render the GUI with React Testing Library against a mocked backend; the reports come from the fixtures' `expected.json` |

Things worth knowing:

- **No access to your real configuration.** Tests build their own `ScanContext` or use fixtures; they never read your home directory's agent configuration.
- **Fixtures** simulate a whole system: `fixture.json` names the operating system, home directory and environment; every file of the simulated system lives below `system/` (e.g. `system/home/alice/.claude/settings.json`, `system/C/Users/alice/...` for Windows). Fixtures contain only invented paths and values. After an intended change to the scanner, regenerate the expectations and review the diff:

  ```bash
  UPDATE_EXPECT=1 cargo test -p inspector-core --test claude_code_fixtures
  UPDATE_EXPECT=1 cargo test -p inspector-core --test codex_fixtures
  git diff fixtures/
  ```

  Git cannot store `.git` directories or symlinks portably, so cases that need them are unit tests with temporary directories.
- **Temporary directories.** Tests that need real files create a uniquely named directory in the system temp directory (`test_support::TempDir`) and delete it afterwards.
- **Platform-specific tests.** Some tests only exist on some systems, e.g. symlink tests are marked `#[cfg(unix)]` because creating symlinks on Windows needs special privileges. Code used only by such tests can trigger "unused" warnings on the other systems (see 6.3).
- **Cross-platform logic is tested everywhere.** Windows path handling, for example, is tested on Linux and macOS too, because the code uses its own target path type instead of the host's path rules (see [architecture](architecture.md), section 5).

## 5. Checks before every commit

These are exactly the checks CI runs:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo deny check          # when dependencies changed
pnpm typecheck && pnpm lint && pnpm test
pnpm audit                # when frontend dependencies changed
```

`--all-targets` makes Clippy check test code too, not only the library and binary.

While iterating, `cargo check` and a targeted test (`cargo test -p inspector-core --test claude_code_fixtures`, or a test name filter) are much faster; run the full checks once at the end of a step.

### Checking other platforms locally

Code behind `#[cfg(unix)]`, `#[cfg(windows)]` and similar is only compiled for the matching platform. To find platform-specific Clippy errors before pushing, check the other targets as well (one-time setup with `rustup target add`):

```bash
rustup target add x86_64-pc-windows-gnu x86_64-unknown-linux-gnu
cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings
cargo clippy --workspace --all-targets --locked --target x86_64-unknown-linux-gnu -- -D warnings
```

This only *checks* the code for the other platform; it does not run the tests there. Running tests on all platforms is what CI is for. The GUI crate cannot be cross-checked from macOS (its build script needs the target's resource compiler or system libraries); add `--exclude inspector-gui` and rely on CI for it.

## 6. Continuous integration

### 6.1 Where it is defined

CI is a single file: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml). It runs on **GitHub Actions**, i.e. on virtual machines provided by GitHub. Nothing runs on your machine.

It starts on every push to `main`, on every pull request and once a week (Monday 06:00 UTC). Pushes and pull requests that only change Markdown files or `docs/` are skipped (`paths-ignore`). A full run can also be started by hand under *Actions → CI → Run workflow* (`workflow_dispatch`). The weekly run notices newly published vulnerabilities in dependencies even when the code has not changed. GitHub uses the workflow file from the pushed commit, so a change to the CI configuration is tested by the same push.

### 6.2 What runs where

A run consists of **jobs**. Each job gets a fresh virtual machine (a *runner*), and all jobs start in parallel:

```text
git push
   │
   ▼
GitHub Actions
   ├── Format                        → Ubuntu runner
   ├── Dependencies (cargo deny)     → Ubuntu runner
   ├── Clippy and tests (macos)      → macOS runner (Apple Silicon)
   ├── Clippy and tests (ubuntu)     → Ubuntu runner
   ├── Clippy and tests (windows)    → Windows runner
   └── Frontend                      → Ubuntu runner
```

The three "Clippy and tests" jobs come from one definition with a **matrix** over `macos-latest`, `windows-latest` and `ubuntu-latest`. `fail-fast: false` lets all three finish even if one fails, so you always see which platforms pass.

Steps of a "Clippy and tests" job:

1. **`actions/checkout`** fetches the pushed commit onto the runner.
2. **`rustup show`** installs the toolchain from `rust-toolchain.toml` (rustup is preinstalled on the runners).
3. **`Swatinem/rust-cache`** restores downloaded crates and build artifacts from earlier runs. The cache key includes `Cargo.lock`, so changed dependencies lead to a fresh build. This mainly speeds up Windows.
4. **Tauri system packages** (Ubuntu only): WebKitGTK and related libraries the GUI crate links against.
5. **Clippy** with `-D warnings` and `--locked` (see 3.3 and 3.5).
6. **Tests** with `--locked`.

The format check and the dependency audit (`EmbarkStudios/cargo-deny-action`, see 3.4) run only once, on Ubuntu, because their results do not depend on the platform. So does the **Frontend** job: `pnpm install --frozen-lockfile`, type check, ESLint, Vitest, production build and `pnpm audit`.

Further settings in the workflow:

- **`concurrency`** with `cancel-in-progress`: if you push twice in a row, the older run on the same branch is cancelled.
- **`permissions: contents: read`**: the workflow can only read the repository. It cannot push, create releases or comment, and it uses no secrets.

Releases are built by a separate workflow, [`release.yml`](../.github/workflows/release.yml), triggered by a `v*` tag; see [release-process.md](release-process.md).

Runners for public repositories are free of charge.

**Dependabot** ([`.github/dependabot.yml`](../.github/dependabot.yml)) is not part of CI but a GitHub service: once a week it checks Cargo and npm dependencies and GitHub Actions for new versions and opens one grouped pull request per ecosystem. These pull requests go through CI like any other.

### 6.3 Why three operating systems

The same source code compiles and behaves differently per platform:

- `#[cfg(unix)]`/`#[cfg(windows)]` code exists only on some platforms. Example: a field read only by a Unix-only test is "never read" on Windows, which Clippy turns into an error there but not on macOS.
- At runtime, path separators, drive letters, letter case and symlink permissions differ. The CLI integration tests, for example, use `C:\Users\alice` as home directory on Windows.

### 6.4 Reading the results

- **Browser:** the *Actions* tab of the repository, or the green check / red cross next to a commit.
- **Terminal:**

  ```bash
  gh run list                        # recent runs
  gh run watch                       # follow a running run
  gh run view <run-id>               # jobs and their status
  gh run view <run-id> --log-failed  # logs of the failed steps only
  ```

"Annotations" in a run (e.g. notices about runner images or deprecated Node.js versions in actions) come from GitHub itself and are not caused by the project's code. They are worth reading when updating action versions.

### 6.5 Common failures

| Symptom in CI | Cause | Fix |
|---|---|---|
| Format job fails | code not formatted | `cargo fmt --all`, commit |
| Clippy fails on one platform only | platform-specific code (see 6.3) | reproduce with `--target` (section 5), fix, push |
| Dependencies job fails | new advisory, disallowed license or source | read the report; update the crate (`cargo update -p <crate>`) or replace it |
| "the lock file needs to be updated" | `Cargo.lock` does not match `Cargo.toml` | `cargo build`, commit `Cargo.lock` |
| `gui_catalog_is_in_sync` fails | core catalog changed without regenerating the GUI file | `UPDATE_EXPECT=1 cargo test -p inspector-core gui_catalog`, add the German text |
| Frontend job: `pnpm audit` fails | new advisory in an npm package | update the package; for a transitive dev dependency, an `overrides` entry in `pnpm-workspace.yaml` |
| Test fails on Windows only | path separators, letter case, `C:\` paths | test with Windows paths in the target path type; check `#[cfg]` usage |

## 7. Where to go next

| Document | Content |
|---|---|
| [roadmap.md](roadmap.md) | goals, decisions, phases and status |
| [architecture.md](architecture.md) | crates, scan flow, scan context, path analysis, CLI |
| [findings-model.md](findings-model.md) | data model and JSON format |
| [release-process.md](release-process.md) | how releases are built and published |
| [claude-code-scanner.md](claude-code-scanner.md) | specification of the Claude Code scanner |
| [adding-a-scanner.md](adding-a-scanner.md) | step-by-step guide for a scanner for a new agent |
| [`CONTRIBUTING.md`](../CONTRIBUTING.md) | setup, checks and rules for pull requests |
| [`CLAUDE.md`](../CLAUDE.md) | working rules (language, privacy, commits), also used by Claude Code |
