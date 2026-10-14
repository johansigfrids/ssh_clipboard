# CI

## Purpose
Document the GitHub Actions setup for tests and releases, including build variants and required system packages.

## Key Files
- `.github/workflows/ci.yml` (PR/push checks)
- `.github/workflows/release.yml` (tagged release builds)
- `.github/workflows/desktop-release.yml` (manual environment-gated Windows signed drafts)
- `.github/workflows/server-release.yml` (manual environment-gated Linux x64 signed drafts)
- `docs/testing.md` (local test commands and Linux deps)

## CI Overview
- `ci.yml` runs on PRs and pushes to `master`:
  - `cargo fmt -- --check`
  - `cargo clippy --features agent -- -D warnings`
  - `cargo test --features agent`
  - `cargo build --release --features agent`
- Matrix variants:
  - `windows-agent`
  - `mac-agent` (host default architecture on macOS runner)
  - `linux-agent`
  - `linux-server` (`--no-default-features`)

The server job installs no GUI prerequisites and rejects OS clipboard/tray/hotkey/X11/Wayland/GTK dependencies in its normal Cargo tree before building. The `system-clipboard` feature can enable native clipboard access without the legacy agent; see [build variants](build-variants.md).

Linux-only maintenance code must pass warnings-as-errors on the CI toolchain as well as Windows checks. Use `Err(eyre!(...))` for match-result expressions, or a statement block with `bail!(...);`: the pinned eyre macro emits a trailing semicolon that Rust rejects in expression position. Older local Linux toolchains may miss this diagnostic.

Process-backed timeout fixtures must allow Windows PowerShell to start and drain their synthetic stderr before the operation assertion. The persistent-connection uncertainty probe uses the shared ten-second fixture budget rather than three seconds; it still requires an unacknowledged mutation timeout, retained epoch and reaped child. Production timeout defaults are unchanged.

## Desktop Prototype Checks

The `desktop` job checks the isolated Phase 1 prototype on Windows, macOS, and Linux. It uses Node.js 24, the committed npm/Cargo lockfiles, Svelte type/build checks, rendered frontend interaction tests, Rust formatting/lint/tests, and `npm run build:prototype` for a standalone executable with an embedded-asset check. Linux installs WebKitGTK 4.1 and tray/clipboard build dependencies. This job does not publish artifacts or run a native GUI session; OS clipboard/paste and focus/shortcut behavior need separate evidence in [desktop-prototype.md](desktop-prototype.md).

## Release Overview
- `release.yml` runs on tags matching `v*`.
- Builds five variants:
  - `windows-agent`
  - `mac-agent-arm64` (`aarch64-apple-darwin`)
  - `mac-agent-x86_64` (`x86_64-apple-darwin`)
  - `linux-agent`
  - `linux-server` (`--no-default-features`)
- Uses explicit `--target` builds so artifact names match architecture deterministically.
- Packages artifacts per OS and uploads to GitHub Releases with SHA256 checksums.
  - Agent builds include both `ssh_clipboard` and `ssh_clipboard_agent`.
- Uses concurrency per tag (`github.ref`) to avoid overlapping release runs.
- Sets `make_latest: false` so a legacy archive release cannot displace the desktop keyed-feed release; legacy downloads use an explicit `v*` tag.

## Desktop Release Staging

Desktop CI also lints/tests installed features without release credentials and builds a synthetic-key NSIS package on Windows without installing it. The mock framework handoff and bounded download probes run as tests. The separate manual desktop workflow pins actions/toolchain, validates an immutable master commit, and exposes signing credentials only in its protected packaging step. It creates drafts; the owner has configured environment protections/signing inputs, while actual protected signing, publishing and installed/native acceptance remain separate. See [release operations](releasing.md).

## Server Release Staging

The manual server workflow validates the exact stable core commit on master, pins Rust/actions, and runs headless checks without credentials. A separate `server-release` environment gates the Linux x64 signing step. It builds on Ubuntu 24.04, verifies exact signed bytes/version/platform with the server policy, and creates a `server-v<version>` draft without publishing/overwriting or becoming latest. The credential-free validation job also executes the actual draft handoff with inert adapters, checking source/platform/profile refusal, no overwrite and desktop-feed isolation. Source SHA/profile/key ID/checksum evidence accompanies the package. Other architectures/runtime compatibility and real user-systemd acceptance remain gates. Environment protections/signing inputs are configured, but this workflow has not run on GitHub; the real staging helper has passed locally with synthetic trust on Windows/Ubuntu WSL. See [release setup](releasing.md#signed-server-drafts).

## Update Triggers
- Changing build variants, features, or supported OSes.
- Adding/removing system package dependencies.
- Modifying artifact naming or release packaging steps.

## Related Docs
- `docs/testing.md`
