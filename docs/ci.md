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
  - `mac-agent`
  - `linux-agent`
  - `linux-server` (`--no-default-features`)

## Release Overview
- `release.yml` runs on tags matching `v*`.
- Builds the same four variants as CI.
- Packages artifacts per OS and uploads to GitHub Releases with SHA256 checksums.

## Update Triggers
- Changing build variants, features, or supported OSes.
- Adding/removing system package dependencies.
- Modifying artifact naming or release packaging steps.

## Related Docs
- `docs/testing.md`
- `IMPLEMENTATION_PLAN.md`
