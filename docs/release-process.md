# Release process

Releases are built and published by [`.github/workflows/release.yml`](../.github/workflows/release.yml). A release contains the desktop application (GUI) and the command line tool (CLI) for macOS, Windows and Linux.

## 1. What a release contains

GUI installers, built with `pnpm tauri build` in the job `gui`:

| File | Platform |
|---|---|
| `ai-agent-access-inspector-gui-<version>-macos-universal.dmg` | macOS 11+, Apple Silicon and Intel |
| `ai-agent-access-inspector-gui-<version>-windows-x64-setup.exe` | Windows x64, installer (NSIS) |
| `ai-agent-access-inspector-gui-<version>-windows-x64.msi` | Windows x64, MSI package |
| `ai-agent-access-inspector-gui-<version>-linux-x86_64.AppImage` | Linux x64, portable (built on Ubuntu 22.04) |
| `ai-agent-access-inspector-gui-<version>-linux-amd64.deb` | Debian, Ubuntu and derivatives |

CLI archives, built in the job `build`:

| Archive | Platform |
|---|---|
| `ai-agent-access-inspector-<version>-aarch64-apple-darwin.tar.gz` | macOS, Apple Silicon |
| `ai-agent-access-inspector-<version>-x86_64-apple-darwin.tar.gz` | macOS, Intel |
| `ai-agent-access-inspector-<version>-x86_64-unknown-linux-musl.tar.gz` | Linux x86_64, statically linked (no glibc requirement) |
| `ai-agent-access-inspector-<version>-x86_64-pc-windows-msvc.zip` | Windows x64 |
| `SHA256SUMS` | SHA-256 checksums of all files |

Each CLI archive contains the binary, `LICENSE`, `README.md` and `CHANGELOG.md`.

Builds are **not signed** yet (roadmap, section 17.4). The macOS app is signed ad hoc (`bundle.macOS.signingIdentity: "-"` in `src-tauri/tauri.conf.json`), which lets it run on Apple Silicon but does not satisfy Gatekeeper; Windows builds carry no signature. The README explains how users open them (*Open Anyway* on macOS, *Run anyway* in SmartScreen); for the CLI on macOS, `xattr -d com.apple.quarantine ai-agent-access-inspector` removes the quarantine attribute. Real signing (Apple Developer ID with notarisation, a Windows code signing certificate, both as GitHub secrets) is planned for `1.0.0`.

Versions `0.x` are published as pre-releases.

## 2. Making a release

1. Make sure `main` is green in CI.
2. Set the version in the root `Cargo.toml` (`[workspace.package] version`) **and** in `package.json`, then run `cargo build` so `Cargo.lock` is updated. The GUI takes its version from `Cargo.toml`; the workflow fails if `package.json` differs.
3. In `CHANGELOG.md`, rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD` and add a new empty `## [Unreleased]` section above it. The release notes are taken from this section.
4. Commit these changes ("Release vX.Y.Z") and push.
5. Recommended: run the workflow manually (*Actions → Release → Run workflow*, or `gh workflow run release.yml`). This builds all archives and installers without publishing anything; they are attached to the workflow run. Install at least the installer for your own system from there.
6. Tag and push:

   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

The workflow then

1. builds and tests the CLI and the GUI on macOS, Linux and Windows,
2. packages the archives and installers,
3. checks that the tag matches the version in `Cargo.toml` and `package.json` and that `CHANGELOG.md` has a section for it (otherwise it fails before publishing),
4. creates `SHA256SUMS`,
5. creates the GitHub release with all files, the checksums and the changelog section as notes.

## 3. Verifying a download

```bash
shasum -a 256 -c SHA256SUMS --ignore-missing    # macOS
sha256sum -c SHA256SUMS --ignore-missing        # Linux
```

On Windows: `Get-FileHash <file> -Algorithm SHA256` and compare with `SHA256SUMS`.

The checksums prove that a download matches what the workflow built, not who built it; that is what signing will add.

## 4. Permissions

The build jobs only read the repository. Only the final publish job has `contents: write`, and only when the workflow was started by a `v*` tag. It uses the workflow's own `GITHUB_TOKEN`; no other secrets are involved. No third-party release action is used: the GUI is built with the project's own `@tauri-apps/cli` from `pnpm-lock.yaml`, and the release is created with `gh`. Tags can only be pushed by people with write access to the repository.

## 5. If something goes wrong

- **Build fails:** fix on `main`, delete the tag locally and on GitHub (`git tag -d vX.Y.Z`, `git push origin :refs/tags/vX.Y.Z`), tag again.
- **Release published with a problem:** do not replace artifacts of a published release; publish a new patch version instead.
