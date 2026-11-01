# Release process

Releases are built and published by [`.github/workflows/release.yml`](../.github/workflows/release.yml). Until the GUI exists (phase 5), a release contains the CLI only.

## 1. What a release contains

| Archive | Platform |
|---|---|
| `ai-agent-access-inspector-<version>-aarch64-apple-darwin.tar.gz` | macOS, Apple Silicon |
| `ai-agent-access-inspector-<version>-x86_64-apple-darwin.tar.gz` | macOS, Intel |
| `ai-agent-access-inspector-<version>-x86_64-unknown-linux-musl.tar.gz` | Linux x86_64, statically linked (no glibc requirement) |
| `ai-agent-access-inspector-<version>-x86_64-pc-windows-msvc.zip` | Windows x64 |
| `SHA256SUMS` | SHA-256 checksums of all archives |

Each archive contains the binary, `LICENSE`, `README.md` and `CHANGELOG.md`.

Builds are **not signed** yet (see roadmap, section 17.4). On macOS, Gatekeeper blocks a downloaded unsigned binary until the quarantine attribute is removed (`xattr -d com.apple.quarantine ai-agent-access-inspector`); on Windows, SmartScreen may warn.

Versions `0.x` are published as pre-releases.

## 2. Making a release

1. Make sure `main` is green in CI.
2. Set the version in the root `Cargo.toml` (`[workspace.package] version`) and run `cargo build` so `Cargo.lock` is updated.
3. In `CHANGELOG.md`, rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD` and add a new empty `## [Unreleased]` section above it. The release notes are taken from this section.
4. Commit these changes ("Release vX.Y.Z") and push.
5. Optional: run the workflow manually (*Actions → Release → Run workflow*, or `gh workflow run release.yml`). This builds all archives without publishing anything; the archives are attached to the workflow run.
6. Tag and push:

   ```bash
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```

The workflow then

1. builds and tests on macOS, Linux and Windows,
2. packages the archives,
3. checks that the tag matches the version in `Cargo.toml` and that `CHANGELOG.md` has a section for it (otherwise it fails before publishing),
4. creates `SHA256SUMS`,
5. creates the GitHub release with the archives, the checksums and the changelog section as notes.

## 3. Verifying a download

```bash
shasum -a 256 -c SHA256SUMS --ignore-missing    # macOS
sha256sum -c SHA256SUMS --ignore-missing        # Linux
```

On Windows: `Get-FileHash <archive> -Algorithm SHA256` and compare with `SHA256SUMS`.

## 4. Permissions

The build jobs only read the repository. Only the final publish job has `contents: write`, and only when the workflow was started by a `v*` tag. It uses the workflow's own `GITHUB_TOKEN`; no other secrets are involved. Tags can only be pushed by people with write access to the repository.

## 5. If something goes wrong

- **Build fails:** fix on `main`, delete the tag locally and on GitHub (`git tag -d vX.Y.Z`, `git push origin :refs/tags/vX.Y.Z`), tag again.
- **Release published with a problem:** do not replace artifacts of a published release; publish a new patch version instead.
