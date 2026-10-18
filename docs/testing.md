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
Default clipboard CLI/core tests (the old agent requires explicit opt-in):
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

The agreed Windows/Apple Silicon/Linux-server delivery and its manual campaigns are closed. Linux desktop and further broad rare-scenario checks are deferred and non-blocking; see the [implementation plan](implementation-plan.md#future-work-and-verification-budget). Prefer existing automated/process/artifact checks. For a concrete change or observed native failure that needs human evidence, state a finite checklist and expected duration, then guide one action at a time. Do not reopen completed campaigns or append optional micro-checkpoints.

The one-time [import review](configuration-migration.md#one-time-import-review) adds rendered discoverability/inert-content/dirty-draft/failure checks and temporary-store acknowledgment probes. These verify retained settings and exact recovery bytes, old receipt compatibility, stale-writer rejection, idempotent dismissal and reload persistence without touching user settings or OS registrations. Synthetic-import review/dismissal/relaunch now have [Windows native evidence](native-history-acceptance.md); actual user-file import and installed cutover remain separate.

The completed daily-use session includes optional direct shortcuts. [Shortcut evidence](desktop-shortcuts.md#evidence-and-remaining-checks) separates automated registry/recorder probes from native background transfers, focus suppression, and occupied-key checks.

## Transport and Diagnostic Faults

Pipe fixtures launch noninteractive PowerShell on Windows and `/bin/sh` on Unix; they never run SSH or access a clipboard. They cover flood/EOF/deadline/reaping and invalid-reply cases with at most two fixture processes at once. Core probes verify that reports expose no sentinel/private output and distinguish unverified layers from passed checks. The real authorized-server probe is metadata-only and separate from these fixtures. See [diagnostic evidence](connection-diagnostics.md#evidence).

The optional V2 shelf adapter is tested with mixed stream clients and both real Linux proxy processes: unknown legacy origin, exact text/PNG bytes and capture timestamps, latest-item selection, shared clear/admission, size refusal with metadata access, malformed/trailing frames, incomplete-upload cleanup, and refusal to detach an occupied legacy endpoint. These checks use synthetic data in disposable local sockets; they do not replace the installed server or prove SSH cutover.

[Released compatibility probes](released-compatibility.md) add the actual pinned v0.3.0 Linux x86_64 executable to both protocol upgrade orders, with exact text/PNG, history deletion/clear, fixed legacy-route refusal and complete response-budget checks. They passed in WSL on 2026-10-16. CI runs the same probe against its optimized Linux server build and retains the metadata-only report. Seven synthetic guard tests cover archive/input admission, redirects, captured-byte extraction and process-group cleanup. These are real local process exchanges with fixed-route shims, not SSH-key, installed-service or native-clipboard acceptance.

The [new session client](shelf-client.md#evidence-and-gaps) adds hostile reply, pagination, cancellation, and bounded SSH subprocess probes. A native WSL command test runs `shelf` through the real session proxy using a synthetic SSH shim; it proves exact stdout/file exports, no clobbering, metadata-only list, and epoch mutation guards with tracing enabled. It does not contact a real SSH server or the OS clipboard.

Desktop [history evidence](desktop-history.md#evidence-and-remaining-acceptance) covers strict endpoint loading, private label projection, epoch/selection state, endpoint isolation, explicit preview commands, and confirmed removal. A synthetic browser layout check at the actual default/minimum window sizes proves bounded document width, inner scrolling, fixed footer, reachable shelf settings, and visible selection feedback. It does not prove native SSH/paste or live subscriptions. The [closed Windows native pass](native-history-acceptance.md) supplies separate observed behavior; omitted cases remain recorded limits.

[Persistent connection probes](persistent-sessions.md#evidence-and-fixture-guardrails) verify repeated requests on one child, lifetime diagnostic bounds, uncertainty/reaping after missing acknowledgment, reader cancellation, and no replay after daemon restart. Native process cases run serially to avoid fork-inherited executable writers; each case still exercises concurrent clients. Disposable runtimes cap worker threads. The clipboard capture-order probe uses a fake owner, not the user's clipboard.

Desktop cancellation checks add a gated native-owner admission probe, controller wake/drop and stale-ID tests, acknowledgment/write finishing races, and rendered Cancel controls that remain available while the transfer or initial panel snapshot is pending. Stale operation events above JavaScript's safe integer range must not resurrect a completed action. Native Cancel and Open-panel-during-transfer passed in the [closed 2026-10-16 session](native-history-acceptance.md), together with lost-acknowledgment status recovery. The report lists omitted native paths explicitly; the final replacement-marker paste was not completed.

Metadata monitor probes add transient reconnect and authentication-pause checks, blocked old-profile cancellation, bounded backoff, and current-generation delivery. Service/client/native proxy tests cover Watch mutation gaps, bounded unchanged heartbeats, future anchors, and expiry. Desktop metadata updates must preserve a surviving selected preview, reject old sequences and snapshots started before a completed user action, and issue no Fetch/Share/Copy command from background UI reload. Network/SSH/native evidence remain distinct.

The new shelf also has an explicitly ignored real SSH probe, run with `./scripts/Test-LiveShelf.ps1 -Target user@host -Binary target/linux-wsl/debug/ssh_clipboard`. The runner builds the test first, creates an owned `/tmp/ssh-clipboard-integration-*` fixture, copies the selected Linux test binary, starts a private temporary daemon/socket, and verifies protected share/retry, exact Unicode bytes, snapshot, Watch, operation acknowledgment, and deletion on one owned SSH child. The Python fixture stops and removes itself on EOF/stop or a 120-second lifetime bound; the runner verifies directory removal after success. It does not install/replace/restart a configured service or access an OS clipboard. On 2026-10-14 this passed from Windows against the authorized Ubuntu host, including cleanup. Native desktop UI/paste and installed-artifact evidence remain separate.

## Native Windows History Evidence

The [2026-10-16 report](native-history-acceptance.md) records the isolated source/build, synthetic migration, real SSH routing, confirmed text/PNG paste, live metadata/privacy, deletion, legacy switching, direct Copy, cancellation/reconnect and acknowledgment recovery. Cleanup removed the temporary shelf and preserved the original service/settings/startup; the original agent was restored when absent at final inspection. This manual session is closed. Remaining retry/epoch/clear UI observations and installed production migration must not be silently counted as passed or appended as more micro-checkpoints.

## Desktop Update Evidence

Desktop update checks run with `cargo test --manifest-path desktop/src-tauri/Cargo.toml --all-features` and frontend `npm test` from `desktop`. They include synthetic cryptographic faults, bounded declared/chunked/interrupted bodies, one-use local handoff ownership, real framework selection in a mock runtime, inert notes, and explicit install/transfer/recovery guards. `npm run test:package` stages an isolated synthetic-key Windows installer and verifies it without execution; private test keys are removed. These passed on Windows on 2026-10-14. Isolated native Windows clean install, manual 0.3.0-to-0.3.1 replacement, retained configuration/startup and real sign-in now pass. Private built-in Windows 0.3.2-to-0.3.3 replacement, download cancellation, tamper rejection, and the 0.3.4-to-0.3.5 installed signing-key transition passed on 2026-10-15. GUI/CLI versions and retained background startup were independently verified. After the final update, the owner confirmed the saved connection and a text Share/Copy/Notepad round trip after deliberately replacing the local clipboard. Native installation interruption, public GitHub/TLS delivery, OS publisher identity and macOS/Linux packages remain separate gates; the opt-in [native updater rehearsal](desktop-updates.md#isolated-native-updater-rehearsal) stages local release-mode packages without production publication. Fixture guard/network probes run in ordinary Node/Rust tests.

## Server Maintenance Evidence

`node --test desktop/tests/server-release.test.mjs` executes the actual server workflow draft handoff with inert file/API adapters. It verifies immutable source and accepted release identity, refusal to overwrite an existing release, draft-only output, and `--latest=false` so server packages do not displace the desktop feed. These checks use no credentials or real GitHub mutations; actual protected signing remains separate evidence.

The Linux server release-store tests use disposable directories and fake admission/service adapters. They fault restart/health, durable outcome writes, rollback health, and journal recovery while checking retained bytes and path/ownership rejection. Separate real synthetic signatures join the concrete policy to activation/rollback; a read-only CLI process rejects altered/forged inputs without execution. Other tests cover wrong-version health, bounded children, and nonblocking rejection of FIFOs/unsafe records. The complete WSL headless suite and all-target clippy pass.

`node desktop/scripts/smoke-server-maintenance.mjs` builds isolated headless versions with the existing Ubuntu WSL toolchain, signs actual binaries with a disposable key, and exercises the CLI under a temporary Linux HOME. It uses **fake systemctl and real daemon/proxy processes**, plus actual `systemd-analyze --user verify` for unit grammar. Checks include initial enable/restart failure cleanup, signed upgrade, saved custom options, retained executables after download removal, tampering/forgery/replay rejection, failed-restart rollback and modeled interrupted-journal recovery. No production key, real user service or live installation is changed. Actual systemd/login lifecycle, real HTTPS artifacts, installed migration and native desktop checks remain distinct gates; see [maintenance evidence](server-maintenance.md).

`npm run test:server-package` from `desktop` separately invokes the real headless staging helper with synthetic trust, verifies the canonical signed Linux x64 package and rejects mismatched public trust before successful candidate/build evidence. It retains public verification/evidence files. The debug artifact is test-only; private synthetic keys are deleted. The protected production 0.4.0 server workflow and independent artifact checks have since passed; see [live acceptance](linux-cutover-acceptance.md). See [server release setup](releasing.md#signed-server-drafts) and the [cutover gates](compatibility-and-cutover.md).

Add `--ssh-target <authorized-host>` to `node desktop/scripts/smoke-server-maintenance.mjs` for real user-systemd acceptance. This passed on 2026-10-14 with a runtime-only unit, temporary HOME/launcher/private socket, signed debug versions and a deliberately crashing last candidate. A 60-second start-limit fault proved and fixed failed-state reset before rollback; enable/start, upgrade, retained bytes/options, version-bound proxy health and journal recovery pass. Cleanup removes its unit/drop-in/data and compares the installed legacy service identity and SSH route to the baseline. It uses no production signing inputs and changes no installed service. The later exact production-package migration and persistent unit/SSH checks passed; actual live reboot/last-session logout remain unobserved and non-blocking; see [real service boundaries](server-maintenance.md#real-user-systemd-acceptance).

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
