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

Headless Linux automation now also runs in the existing Ubuntu WSL environment: `cargo test --no-default-features --target-dir target/linux-wsl` and matching all-target clippy checks. The [native endpoint probes](shelf-server.md#evidence-and-remaining-work) use disposable processes/sockets and synthetic data, including held-open stdin and duplicate-startup preservation. This is not a GUI/native-paste acceptance pass or a remote deployment.

The standalone shelf core has synthetic [resource/behavior probes](shelf-core.md#evidence-and-integration-work). These do not establish daemon integration or Linux socket/ownership behavior.

Manual Windows testing is grouped at larger milestones: daily clipboard/settings/reconnect use, OS integration/installation, and signed updates/recovery. Continue automated implementation and commit usable slices between these passes. Individual advanced-settings and diagnostic UI checks are deferred into the next daily-use pass; do not ask for a new manual check after every small commit. Focused repeats are appropriate when a native failure needs confirmation. Other platforms still need their own supported-platform evidence.

Optional direct shortcuts join the daily-use pass. [Shortcut evidence](desktop-shortcuts.md#evidence-and-remaining-checks) separates automated registry/recorder probes from native background transfers, focus suppression, and occupied-key checks.

## Transport and Diagnostic Faults

Pipe fixtures launch noninteractive PowerShell on Windows and `/bin/sh` on Unix; they never run SSH or access a clipboard. They cover flood/EOF/deadline/reaping and invalid-reply cases with at most two fixture processes at once. Core probes verify that reports expose no sentinel/private output and distinguish unverified layers from passed checks. The real authorized-server probe is metadata-only and separate from these fixtures. See [diagnostic evidence](connection-diagnostics.md#evidence).

The optional V2 shelf adapter is tested with mixed stream clients and both real Linux proxy processes: unknown legacy origin, exact text/PNG bytes and capture timestamps, latest-item selection, shared clear/admission, size refusal with metadata access, malformed/trailing frames, incomplete-upload cleanup, and refusal to detach an occupied legacy endpoint. These checks use synthetic data in disposable local sockets; they do not replace the installed server or prove SSH cutover.

The [new session client](shelf-client.md#evidence-and-gaps) adds hostile reply, pagination, cancellation, and bounded SSH subprocess probes. A native WSL command test runs `shelf` through the real session proxy using a synthetic SSH shim; it proves exact stdout/file exports, no clobbering, metadata-only list, and epoch mutation guards with tracing enabled. It does not contact a real SSH server or the OS clipboard.

Desktop [history evidence](desktop-history.md#evidence-and-remaining-acceptance) covers strict endpoint loading, private label projection, epoch/selection state, endpoint isolation, explicit preview commands, and confirmed removal. A synthetic browser layout check at the actual default/minimum window sizes proves bounded document width, inner scrolling, fixed footer, reachable shelf settings, and visible selection feedback. It does not prove native SSH/paste or live subscriptions. These checks join the combined daily-use native milestone rather than creating a manual gate per commit.

[Persistent connection probes](persistent-sessions.md#evidence-and-fixture-guardrails) verify repeated requests on one child, lifetime diagnostic bounds, uncertainty/reaping after missing acknowledgment, reader cancellation, and no replay after daemon restart. Native process cases run serially to avoid fork-inherited executable writers; each case still exercises concurrent clients. Disposable runtimes cap worker threads. The clipboard capture-order probe uses a fake owner, not the user's clipboard.

Desktop cancellation checks add a gated native-owner admission probe, controller wake/drop and stale-ID tests, acknowledgment/write finishing races, and rendered Cancel controls that remain available while the transfer or initial panel snapshot is pending. Stale operation events above JavaScript's safe integer range must not resurrect a completed action. Native Cancel and Open-panel-during-transfer checks belong to the next combined acceptance pass; the user-confirmed Windows direct Share result is recorded in the shortcut evidence, separately from these automated checks.

Metadata monitor probes add transient reconnect and authentication-pause checks, blocked old-profile cancellation, bounded backoff, and current-generation delivery. Service/client/native proxy tests cover Watch mutation gaps, bounded unchanged heartbeats, future anchors, and expiry. Desktop metadata updates must preserve a surviving selected preview, reject old sequences and snapshots started before a completed user action, and issue no Fetch/Share/Copy command from background UI reload. Network/SSH/native evidence remain distinct.

The new shelf also has an explicitly ignored real SSH probe, run with `./scripts/Test-LiveShelf.ps1 -Target user@host -Binary target/linux-wsl/debug/ssh_clipboard`. The runner builds the test first, creates an owned `/tmp/ssh-clipboard-integration-*` fixture, copies the selected Linux test binary, starts a private temporary daemon/socket, and verifies protected share/retry, exact Unicode bytes, snapshot, Watch, operation acknowledgment, and deletion on one owned SSH child. The Python fixture stops and removes itself on EOF/stop or a 120-second lifetime bound; the runner verifies directory removal after success. It does not install/replace/restart a configured service or access an OS clipboard. On 2026-10-14 this passed from Windows against the authorized Ubuntu host, including cleanup. Native desktop UI/paste and installed-artifact evidence remain separate.

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
