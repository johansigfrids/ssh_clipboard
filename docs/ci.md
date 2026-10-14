# CI

## Purpose
Document the GitHub Actions setup for tests and releases, including build variants and required system packages.

## Key Files
- `.github/workflows/ci.yml` (PR/push checks)
- `.github/workflows/release.yml` (tagged release builds)
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

## Update Triggers
- Changing build variants, features, or supported OSes.
- Adding/removing system package dependencies.
- Modifying artifact naming or release packaging steps.

## Related Docs
- `docs/testing.md`
