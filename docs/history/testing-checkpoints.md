# Development Testing Checkpoints

> Historical snapshot archived 2026-10-18. Counts, installed-state claims and pending gates below belong to their recorded builds/sessions. Use [current testing guidance](../testing.md), the [closed scope](implementation-plan.md) and later acceptance reports for current decisions.

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
- [Desktop prototype guide](../desktop-development.md) (separate frontend/backend checks and native acceptance gate)

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
- **Headless boundary**: shared PNG/action tests run without `arboard`; executable checks reject unsupported OS clipboard actions before SSH. CI guards the Linux dependency tree against desktop libraries. See [build variants](../build-variants.md).
- **Migration**: exact original/backup bytes, schema 1 read-only upgrade and first-write backup, schema 2 strictness, current-agent serialization, repeat import, backup conflict/retry, shared-file precedence, stable device IDs, advanced overrides, and retained startup/direct-shortcut choices. These use temporary files; they do not import the user's real agent file. See [migration/recovery](../configuration-migration.md).
- **Desktop profiles**: real temporary-store create/rename/select/remove flows, independent defaults, retained global preferences and device identity, snapshot clearing for same-target profiles, stale-save rejection before activation, abandoned saves, and replacement rollback. Rendered controls check draft selection, creation cancellation, confirmation, removal failure, and external-deletion recovery. Native profile controls remain a separate smoke test.
- **Advanced connection settings**: lossless decimal-string IPC beyond JavaScript's exact integer range; numeric/control-character rejection; stored policy used by actual client configuration; retained batch mode; nested-draft isolation; multiline editing with blank lines; reset/Discard; strict-mode budget retention; external-edit recovery. These do not prove native controls or change a running server's policy.

## Notes
- Some tests are Linux-only (`#[cfg(test)]` in `src/daemon.rs`).
- Property tests use `proptest`; keep bounds small to avoid long test runs.
- The legacy CI job runs four variants: Windows agent, macOS agent, Linux agent, and Linux server (`--no-default-features`). Separate desktop prototype jobs check Windows/macOS/Linux without proving native paste or shortcut behavior.
- CI installs Linux GUI deps so the agent feature can build.

## Native Test Scheduling

Headless Linux automation now also runs in the existing Ubuntu WSL environment: `cargo test --no-default-features --target-dir target/linux-wsl` and matching all-target clippy checks. The [native endpoint probes](../shelf-server.md#validation) use disposable processes/sockets and synthetic data, including held-open stdin and duplicate-startup preservation. This is not a GUI/native-paste acceptance pass or a remote deployment.

The standalone shelf core has synthetic [resource/behavior probes](../shelf-core.md#validation). These do not establish daemon integration or Linux socket/ownership behavior.

The agreed Windows/Apple Silicon/Linux-server delivery and its manual campaigns are closed. Linux desktop and further broad rare-scenario checks are deferred and non-blocking; see the [implementation plan](implementation-plan.md#future-work-and-verification-budget). Prefer existing automated/process/artifact checks. For a concrete change or observed native failure that needs human evidence, state a finite checklist and expected duration, then guide one action at a time. Do not reopen completed campaigns or append optional micro-checkpoints.

The one-time [import review](../configuration-migration.md#one-time-import-review) adds rendered discoverability/inert-content/dirty-draft/failure checks and temporary-store acknowledgment probes. These verify retained settings and exact recovery bytes, old receipt compatibility, stale-writer rejection, idempotent dismissal and reload persistence without touching user settings or OS registrations. Synthetic-import review/dismissal/relaunch now have [Windows native evidence](native-history-acceptance.md); actual user-file import and installed cutover remain separate.

The completed daily-use session includes optional direct shortcuts. [Shortcut evidence](../desktop-shortcuts.md#evidence-and-remaining-checks) separates automated registry/recorder probes from native background transfers, focus suppression, and occupied-key checks.

## Transport and Diagnostic Faults

Pipe fixtures launch noninteractive PowerShell on Windows and `/bin/sh` on Unix; they never run SSH or access a clipboard. They cover flood/EOF/deadline/reaping and invalid-reply cases with at most two fixture processes at once. Core probes verify that reports expose no sentinel/private output and distinguish unverified layers from passed checks. The real authorized-server probe is metadata-only and separate from these fixtures. See [diagnostic evidence](../connection-diagnostics.md#evidence).

The optional V2 shelf adapter is tested with mixed stream clients and both real Linux proxy processes: unknown legacy origin, exact text/PNG bytes and capture timestamps, latest-item selection, shared clear/admission, size refusal with metadata access, malformed/trailing frames, incomplete-upload cleanup, and refusal to detach an occupied legacy endpoint. These checks use synthetic data in disposable local sockets; they do not replace the installed server or prove SSH cutover.

[Released compatibility probes](../released-compatibility.md) add the actual pinned v0.3.0 Linux x86_64 executable to both protocol upgrade orders, with exact text/PNG, history deletion/clear, fixed legacy-route refusal and complete response-budget checks. They passed in WSL on 2026-10-16. CI runs the same probe against its optimized Linux server build and retains the metadata-only report. Seven synthetic guard tests cover archive/input admission, redirects, captured-byte extraction and process-group cleanup. These are real local process exchanges with fixed-route shims, not SSH-key, installed-service or native-clipboard acceptance.

The [new session client](../shelf-client.md#evidence-and-gaps) adds hostile reply, pagination, cancellation, and bounded SSH subprocess probes. A native WSL command test runs `shelf` through the real session proxy using a synthetic SSH shim; it proves exact stdout/file exports, no clobbering, metadata-only list, and epoch mutation guards with tracing enabled. It does not contact a real SSH server or the OS clipboard.

Desktop [history evidence](../desktop-history.md#evidence-and-remaining-acceptance) covers strict endpoint loading, private label projection, epoch/selection state, endpoint isolation, explicit preview commands, and confirmed removal. A synthetic browser layout check at the actual default/minimum window sizes proves bounded document width, inner scrolling, fixed footer, reachable shelf settings, and visible selection feedback. It does not prove native SSH/paste or live subscriptions. The [closed Windows native pass](native-history-acceptance.md) supplies separate observed behavior; omitted cases remain recorded limits.

[Persistent connection probes](../persistent-sessions.md#evidence-and-fixture-guardrails) verify repeated requests on one child, lifetime diagnostic bounds, uncertainty/reaping after missing acknowledgment, reader cancellation, and no replay after daemon restart. Native process cases run serially to avoid fork-inherited executable writers; each case still exercises concurrent clients. Disposable runtimes cap worker threads. The clipboard capture-order probe uses a fake owner, not the user's clipboard.

Desktop cancellation checks add a gated native-owner admission probe, controller wake/drop and stale-ID tests, acknowledgment/write finishing races, and rendered Cancel controls that remain available while the transfer or initial panel snapshot is pending. Stale operation events above JavaScript's safe integer range must not resurrect a completed action. Native Cancel and Open-panel-during-transfer passed in the [closed 2026-10-16 session](native-history-acceptance.md), together with lost-acknowledgment status recovery. The report lists omitted native paths explicitly; the final replacement-marker paste was not completed.

Metadata monitor probes add transient reconnect and authentication-pause checks, blocked old-profile cancellation, bounded backoff, and current-generation delivery. Service/client/native proxy tests cover Watch mutation gaps, bounded unchanged heartbeats, future anchors, and expiry. Desktop metadata updates must preserve a surviving selected preview, reject old sequences and snapshots started before a completed user action, and issue no Fetch/Share/Copy command from background UI reload. Network/SSH/native evidence remain distinct.

The new shelf also has an explicitly ignored real SSH probe, run with `./scripts/Test-LiveShelf.ps1 -Target user@host -Binary target/linux-wsl/debug/ssh_clipboard`. The runner builds the test first, creates an owned `/tmp/ssh-clipboard-integration-*` fixture, copies the selected Linux test binary, starts a private temporary daemon/socket, and verifies protected share/retry, exact Unicode bytes, snapshot, Watch, operation acknowledgment, and deletion on one owned SSH child. The Python fixture stops and removes itself on EOF/stop or a 120-second lifetime bound; the runner verifies directory removal after success. It does not install/replace/restart a configured service or access an OS clipboard. On 2026-10-14 this passed from Windows against the authorized Ubuntu host, including cleanup. Native desktop UI/paste and installed-artifact evidence remain separate.

## Native Windows History Evidence

The [2026-10-16 report](native-history-acceptance.md) records the isolated source/build, synthetic migration, real SSH routing, confirmed text/PNG paste, live metadata/privacy, deletion, legacy switching, direct Copy, cancellation/reconnect and acknowledgment recovery. Cleanup removed the temporary shelf and preserved the original service/settings/startup; the original agent was restored when absent at final inspection. This manual session is closed. Remaining retry/epoch/clear UI observations and installed production migration must not be silently counted as passed or appended as more micro-checkpoints.

## Desktop Update Evidence

Desktop update checks run with `cargo test --manifest-path desktop/src-tauri/Cargo.toml --all-features` and frontend `npm test` from `desktop`. They include synthetic cryptographic faults, bounded declared/chunked/interrupted bodies, one-use local handoff ownership, real framework selection in a mock runtime, inert notes, and explicit install/transfer/recovery guards. `npm run test:package` stages an isolated synthetic-key Windows installer and verifies it without execution; private test keys are removed. These passed on Windows on 2026-10-14. Isolated native Windows clean install, manual 0.3.0-to-0.3.1 replacement, retained configuration/startup and real sign-in now pass. Private built-in Windows 0.3.2-to-0.3.3 replacement, download cancellation, tamper rejection, and the 0.3.4-to-0.3.5 installed signing-key transition passed on 2026-10-15. GUI/CLI versions and retained background startup were independently verified. After the final update, the owner confirmed the saved connection and a text Share/Copy/Notepad round trip after deliberately replacing the local clipboard. Native installation interruption, public GitHub/TLS delivery, OS publisher identity and macOS/Linux packages remain separate gates; the opt-in [native updater rehearsal](../desktop-updates.md#isolated-native-updater-rehearsal) stages local release-mode packages without production publication. Fixture guard/network probes run in ordinary Node/Rust tests.

## Server Maintenance Evidence

`node --test desktop/tests/server-release.test.mjs` executes the actual server workflow draft handoff with inert file/API adapters. It verifies immutable source and accepted release identity, refusal to overwrite an existing release, draft-only output, and `--latest=false` so server packages do not displace the desktop feed. These checks use no credentials or real GitHub mutations; actual protected signing remains separate evidence.

The Linux server release-store tests use disposable directories and fake admission/service adapters. They fault restart/health, durable outcome writes, rollback health, and journal recovery while checking retained bytes and path/ownership rejection. Separate real synthetic signatures join the concrete policy to activation/rollback; a read-only CLI process rejects altered/forged inputs without execution. Other tests cover wrong-version health, bounded children, and nonblocking rejection of FIFOs/unsafe records. The complete WSL headless suite and all-target clippy pass.

`node desktop/scripts/smoke-server-maintenance.mjs` builds isolated headless versions with the existing Ubuntu WSL toolchain, signs actual binaries with a disposable key, and exercises the CLI under a temporary Linux HOME. It uses **fake systemctl and real daemon/proxy processes**, plus actual `systemd-analyze --user verify` for unit grammar. Checks include initial enable/restart failure cleanup, signed upgrade, saved custom options, retained executables after download removal, tampering/forgery/replay rejection, failed-restart rollback and modeled interrupted-journal recovery. No production key, real user service or live installation is changed. Actual systemd/login lifecycle, real HTTPS artifacts, installed migration and native desktop checks remain distinct gates; see [maintenance evidence](../server-maintenance.md).

`npm run test:server-package` from `desktop` separately invokes the real headless staging helper with synthetic trust, verifies the canonical signed Linux x64 package and rejects mismatched public trust before successful candidate/build evidence. It retains public verification/evidence files. The debug artifact is test-only; private synthetic keys are deleted. The protected production 0.4.0 server workflow and independent artifact checks have since passed; see [live acceptance](linux-cutover-acceptance.md). See [server release setup](../releasing.md#signed-server-drafts) and the [cutover gates](../compatibility-and-cutover.md).

Add `--ssh-target <authorized-host>` to `node desktop/scripts/smoke-server-maintenance.mjs` for real user-systemd acceptance. This passed on 2026-10-14 with a runtime-only unit, temporary HOME/launcher/private socket, signed debug versions and a deliberately crashing last candidate. A 60-second start-limit fault proved and fixed failed-state reset before rollback; enable/start, upgrade, retained bytes/options, version-bound proxy health and journal recovery pass. Cleanup removes its unit/drop-in/data and compares the installed legacy service identity and SSH route to the baseline. It uses no production signing inputs and changes no installed service. The later exact production-package migration and persistent unit/SSH checks passed; actual live reboot/last-session logout remain unobserved and non-blocking; see [real service boundaries](../server-maintenance.md#real-user-systemd-acceptance).

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
- [docs/troubleshooting.md](../troubleshooting.md)
- [docs/cli.md](../cli.md)


## Configuration and Build Checkpoints

These paragraphs were moved from current guides during the 2026-10-18 cleanup. Their original dates and test boundaries remain.

### Shared Configuration

Temporary-directory Rust probes cover restoration, stale writers, competing handles, abandoned preparation, corrupt/future/unknown fields, invalid profile identity/selection, byte budgets, and failed replacement. Desktop mapping tests check profile/preference restoration and preservation of other profiles. Frontend tests check startup restoration without transfers, blocked corrupt-file saves, and failed-save drafts. Executable-level CLI tests check persisted selection, shared-store parity, relative paths, rejected/corrupt edits, and missing-profile failure before SSH/clipboard access; resolution probes cover direct destinations and explicit override precedence.

These tests never access the user's settings or OS clipboard. The user confirmed the native Windows Quit/relaunch flow on 2026-10-13: applied settings are remembered in the persistent-settings artifact. Later Mac migration and isolated updater persistence are recorded in [Mac cutover](macos-cutover.md) and [Mac updater acceptance](macos-updater-acceptance.md).

Live-reload backend probes cover profile isolation, preference-only retention, unchanged-file silence, corrupted-file recovery, rejected OS activation, and deletion back to setup. Frontend probes cover preserving/invalidating drafts, clean-form reload, notifications during startup/a pending transfer, subscription-failure cleanup, and stale transfer guards. Subscribe before the initial state read, queue startup notifications until it completes, and keep controls disabled until ready; otherwise an edit can be missed or an early draft replaced by initialization. Clean up successful subscriptions if another registration fails. On 2026-10-14 the user confirmed the native Windows app/CLI live-reload check: changing preview visibility through CLI updates the open app, and restoring it works. Fault probes still use controlled activation callbacks.

The 2026-10-13 target-less CLI `peek --ssh-option BatchMode=yes --json` also reached the server using the saved desktop profile and returned PNG metadata. This probe fetched no payload and changed neither clipboard.

On 2026-10-14, temporary-store desktop probes cover creating/renaming/removing connections, UUID stability, preserved device/preferences/transport, same-target profile isolation, stale saves, activation rejection, and replacement rollback. Executable CLI checks cover deleting selected/inactive profiles and refusing missing/last profiles without changing bytes. Rendered frontend checks cover draft selection, cancelling creation, confirmation/cancellation, failed removal, and recovery after external deletion. Separately, on 2026-10-14 the user confirmed native Windows creation/Test, switching with preview clearing, rename/relaunch persistence, and removal back to the original connection in SSHClipboardPrototype-20261014T070735201Z.exe.

Advanced-policy probes cover exact integers beyond JavaScript's safe range, rejected zero/negative/fractional/overflowing values and embedded option control characters, per-profile persistence and actual client policy, retained batch mode, isolated nested drafts, multiline option editing, reset/Discard, strict-budget retention, and stale-draft recovery. These are temporary-store/rendered UI checks; native advanced-control persistence/Test remains pending.


### Configuration Migration

On 2026-10-15, rendered frontend and temporary-store/backend tests pass for discoverability before opening Settings, retained binding/startup explanations, inert names/paths, dirty-draft blocking, guarded explicit dismissal, failure retention, reload persistence, idempotency and stale-write refusal. A desktop session probe confirms acknowledgment preserves the active setup and clipboard snapshot; exact source/backup bytes remain unchanged. The shared CLI lockfile also reads older receipts without rewriting them. On 2026-10-16 the owner confirmed the review card, explicit dismissal, and saved dismissal/profile after Quit/relaunch in an isolated Windows build with synthetic legacy settings. See the [closed native session](native-history-acceptance.md). Import of the actual legacy user file and installed startup cutover remain separate.

On 2026-10-14, temporary-file/executable probes pass for exact backup bytes, current-agent serialization, missing/invalid sources, idempotency, backup conflicts/retry, shared-file precedence, schema 1 read-only upgrade/backup, strict schema 2 fields, stable device IDs, advanced-option overrides, and preserved preferences. Desktop probes confirm migrated transport values, forced GUI batch mode, and snapshot clearing when advanced connection options change. Existing version 2 wire fixtures and PNG pixel/ownership contracts still pass.

`tests/fixtures/legacy-agent.toml` is imported under both CLI and desktop dependency lockfiles, which currently select different compatible TOML parser patch releases. Keep that fixture check when changing either graph; a compatible version range alone is not migration evidence. Local OpenSSH `-G` with an empty configuration also confirms that the prepended batch-mode value wins without connecting to a host.

A metadata-only CLI probe through the user's existing prototype profile succeeded. The settings file's SHA-256 was identical before/after, confirming that the saved file was unchanged. No legacy user file was imported by the automated probes, and no server was replaced/restarted. On 2026-10-14 the user confirmed the native Windows schema 2 Apply/Quit/relaunch check and small-image Share/Copy/paste into Paint in `SSHClipboardPrototype-20261014T064745955Z.exe`. This confirms the prototype upgrade path and native RGBA adapter; those early checks do not establish actual-user import on another platform. Later actual Mac import and clean cutover are recorded in [Mac cutover](macos-cutover.md).


### Dependency Split

At the initial dependency-split checkpoint on 2026-10-14, Windows headless (42 unit + 5 executable checks), legacy agent (56 + 4), and desktop backend (18) tests passed, with required lint/format checks. Subsequent migration adds its own probes; see [migration evidence](../configuration-migration.md#evidence). The Linux-target normal dependency tree remains free of OS clipboard/desktop libraries after those additions. This is dependency evidence, not a native Linux build: the authorized test server has no Cargo available in its SSH environment, and no toolchain was installed. CI's Linux compilation/native platform checks remain required. No running server was replaced or restarted.

Subsequent 2026-10-14 evidence closes the headless Linux compilation gap: the existing Ubuntu WSL Rust 1.97.1 toolchain runs native unit, migration, settings/doctor, and real shelf/legacy daemon-proxy process probes without GUI libraries. Output uses `target/linux-wsl` to avoid mixing Windows artifacts. See [native evidence and limits](../shelf-server.md#validation). This does not establish a Linux desktop GUI session or a remote-host rollout.

Mac packaging has a separate CI probe: `npm run test:package -- --release` builds installed features with a disposable updater key and test app identifier, then inspects the archived app/CLI and their code signatures without launching or installing the GUI. It needs no production signing secrets or build tools on the owner's Mac. See [Mac cutover](macos-cutover.md) for scope and pending native gates.

### CI Packaging

On 2026-10-16, a full Windows run exceeded three existing elapsed-time assertions while separately admitted transport modules competed with each other and compilation. The isolated timeout probes passed. Legacy, one-shot session and persistent-session pipe fixtures now acquire the same two-slot test semaphore before spawning, instead of independent module limits. Full Windows and headless WSL suites subsequently passed with their timing/uncertainty/reaping assertions unchanged. Keep admission shared across fixture families; do not hide startup contention by extending production deadlines or dropping cleanup assertions.

The macOS PR job now also builds a release-profile test app with disposable updater trust and an explicit ad-hoc code signature, then checks the extracted bundle identity/version, app/CLI architecture, code signatures and CLI version without starting the GUI. The public package/inspection evidence is retained for seven days after private test-key cleanup. The first probe and all seven jobs passed in [run 38919474529](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38919474529) for PR head `4a05401` (synthetic merge `3c8897a`). A later private actual-Mac CLI/import probe also passed; see [Mac evidence](macos-cutover.md#status). This does not establish GUI installation, paste, startup or updater replacement, and subsequent candidate changes need fresh exact-head CI.
