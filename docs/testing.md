# Testing

## Purpose
Describe how to run tests locally and what kinds of tests exist in this repo.

## Key Files
- `src/framing.rs` (framing unit tests + proptest)
- `src/protocol.rs` (protocol round-trip tests)
- `src/client_actions.rs` (clipboard contract tests)
- `src/client/transport/tests.rs`, `src/client/diagnostics.rs`, `src/client/failure.rs`, `tests/connection_doctor.rs` (real pipe-process faults, safe reports, and metadata-only health)
- `src/configuration.rs` (temporary-directory save, conflict, corruption, and recovery probes)
- `src/configuration/migration.rs`, `tests/configuration_migration.rs` (legacy/schema upgrades, backups, preserved policy and identity)
- `src/cli/settings.rs`, `tests/shared_settings_cli.rs` (profile resolution and executable-level shared settings checks)
- `src/daemon.rs` (Linux-only daemon tests)
- `Cargo.toml` (dev-dependencies for tests)
- [Desktop prototype guide](desktop-prototype.md) (separate frontend/backend checks and native acceptance gate)

## How to Run
Basic unit tests (agent is enabled by default):
```
cargo test
```

CI (runs on GitHub Actions, 4 build variants):
```
cargo fmt -- --check
cargo clippy --features agent -- -D warnings
cargo test --features agent
cargo build --release --features agent
```

With agent feature:
```
cargo test --features agent
```

## Linux Build Dependencies (agent)
Ubuntu packages needed to build the agent on Linux:
```
sudo apt-get update
sudo apt-get install -y \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libappindicator3-dev \
  libdbus-1-dev \
  libxdo-dev \
  libx11-dev \
  libxkbcommon-dev \
  libwayland-dev \
  libxrandr-dev \
  libxinerama-dev \
  libxcursor-dev \
  libxi-dev \
  libxfixes-dev \
  libxcb1-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev
```

## Test Categories
- **Framing/protocol**: round-trip, invalid framing, truncated payloads.
- **Client actions**: behavior parity tests using mock clipboard.
- **Image handling**: PNG encode/decode and size guard tests.
- **Daemon (Linux-only)**: socket path permissions, request validation, read timeouts.
- **Configuration**: restoration, stale writers, competing locks, abandoned preparations, corrupt/future/oversized documents, profile selection, and failed replacement. These use temporary directories and never edit the user's settings.
- **Shared CLI configuration**: JSON/store parity across executable launches, profile addition/selection, direct destination and override precedence, rejected/corrupt edits, relative paths, and missing-profile failure before SSH/clipboard access. Use `--settings-file` with a temporary path so probes never alter real settings.
- **Headless boundary**: shared PNG/action tests run without `arboard`; executable checks reject unsupported OS clipboard actions before SSH. CI guards the Linux dependency tree against desktop libraries. See [build variants](build-variants.md).
- **Migration**: exact original/backup bytes, schema 1 read-only upgrade and first-write backup, schema 2 strictness, current-agent serialization, repeat import, backup conflict/retry, shared-file precedence, stable device IDs, advanced overrides, and retained startup/direct-shortcut choices. These use temporary files; they do not import the user's real agent file. See [migration/recovery](configuration-migration.md).
- **Desktop profiles**: real temporary-store create/rename/select/remove flows, independent defaults, retained global preferences and device identity, snapshot clearing for same-target profiles, stale-save rejection before activation, abandoned saves, and replacement rollback. Rendered controls check draft selection, creation cancellation, confirmation, removal failure, and external-deletion recovery. Native profile controls remain a separate smoke test.
- **Advanced connection settings**: lossless decimal-string IPC beyond JavaScript's exact integer range; numeric/control-character rejection; stored policy used by actual client configuration; retained batch mode; nested-draft isolation; multiline editing with blank lines; reset/Discard; strict-mode budget retention; external-edit recovery. These do not prove native controls or change a running server's policy.

## Notes
- Some tests are Linux-only (`#[cfg(test)]` in `src/daemon.rs`).
- Property tests use `proptest`; keep bounds small to avoid long test runs.
- The legacy CI job runs four variants: Windows agent, macOS agent, Linux agent, and Linux server (`--no-default-features`). Separate desktop prototype jobs check Windows/macOS/Linux without proving native paste or shortcut behavior.
- CI installs Linux GUI deps so the agent feature can build.

## Native Test Scheduling

Manual Windows testing is grouped at larger milestones: daily clipboard/settings/reconnect use, OS integration/installation, and signed updates/recovery. Continue automated implementation and commit usable slices between these passes. Individual advanced-settings and diagnostic UI checks are deferred into the next daily-use pass; do not ask for a new manual check after every small commit. Focused repeats are appropriate when a native failure needs confirmation. Other platforms still need their own supported-platform evidence.

## Transport and Diagnostic Faults

Pipe fixtures launch noninteractive PowerShell on Windows and `/bin/sh` on Unix; they never run SSH or access a clipboard. They cover flood/EOF/deadline/reaping and invalid-reply cases with at most two fixture processes at once. Core probes verify that reports expose no sentinel/private output and distinguish unverified layers from passed checks. The real authorized-server probe is metadata-only and separate from these fixtures. See [diagnostic evidence](connection-diagnostics.md#evidence).

## Platform `cfg` Hygiene
- Keep function/item `#[cfg(...)]` aligned with where it is used.
- If a helper is only called from Windows-gated code, gate the helper too (for example `#[cfg(target_os = "windows")]`).
- If tests need a platform-gated helper on other platforms, use `#[cfg(any(target_os = "windows", test))]`.
- For parameters only used on one platform branch, mark intentional non-use in the other branch (for example `#[cfg(not(target_os = "windows"))] let _ = install_dir;`).
- Before pushing, run the CI lint command locally:
```
cargo clippy --features agent -- -D warnings
```

## Update Triggers
- New features or protocol changes.
- Changes to clipboard handling or agent behavior.
- Adding/removing dev-dependencies for tests.

## Related Docs
- `docs/troubleshooting.md`
- `docs/cli.md`
