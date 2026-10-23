# Testing

## Purpose

Choose checks for the changed component and distinguish automated, process, artifact and native evidence. Exact historical counts and results belong in [testing checkpoints](history/testing-checkpoints.md) and [acceptance reports](history/release-readiness.md).

## Key Files

- `src/`, `tests/`: Rust unit, property, CLI and native-process contracts.
- `desktop/tests/`: Node release/tooling tests and rendered Svelte interactions.
- `desktop/src-tauri/tests/`, `desktop/src-tauri/src/panel/`: backend and framework integration tests.
- `scripts/`: disposable shelf, service and released-binary fixtures.
- `.github/workflows/ci.yml`: cross-platform regression matrix and dependency guards.

## How to Run

From the repository root, choose the affected Rust variants:

```powershell
cargo fmt -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
```

The root build is the session-only headless CLI/server. Linux-only socket/service tests require Linux. On this Windows workspace the established WSL command is:

```powershell
wsl.exe -d Ubuntu -- bash -lc 'cargo test --locked --target-dir target/linux-wsl'
```

From `desktop/`, validate frontend and release helpers:

```powershell
npm test
npm run build
```

From the repository root, validate the separate desktop Rust project:

```powershell
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml -- --check
cargo clippy --locked --manifest-path desktop/src-tauri/Cargo.toml --all-features --all-targets -- -D warnings
cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml --all-features
```

`npm test` runs Node tooling tests and Vitest rendered interactions. `npm run build` runs Svelte checks before Vite. For standalone artifacts, use [desktop development](desktop-development.md); for packaging changes, also use the relevant package smoke below. [CI](ci.md) owns the platform/package prerequisite list. Missing toolchains or caches make a check incomplete, rather than passed.

## Test Categories

| Surface | Observable contracts |
| --- | --- |
| Protocol and shelf | Strict SCBS metadata/raw frames, epoch/correlation, paging, budgets, expiry, deduplication and deadline-protected retry. |
| Configuration | Exact backups, corrupt/future-file preservation, stale writers, retained descriptor locks, profile isolation, atomic save and activation rollback. Use temporary settings paths. |
| CLI | Session-only command surface and removed-command refusal, direct-target precedence, explicit shelf input/export, no-clobber outputs and failure before SSH/clipboard access. |
| Transport | Flood/EOF/deadline/reaping, poisoned sessions, no replay, metadata-only recovery and stale-generation rejection. |
| Desktop | Draft/Apply/Discard, startup/shortcut rollback, preview privacy, selected-item copy, cancellation and retained outcomes. Native adapters are controlled in automated tests. |
| Release | Exact source/profile/platform/bytes, synthetic signatures, bounded downloads, real draft-handoff refusal and package identity. Normal tests need no production secrets. |

## Desktop IPC Contract

`desktop/src-tauri/src/panel/ipc_contract_tests.rs` serializes real initial/populated history views, operation state and update state into the checked-in `desktop/tests/fixtures/ipc.ts`. The generated file uses TypeScript `satisfies` against the frontend interfaces and is included in the normal Svelte check. Rust tests reject unreviewed fixture drift; rendered tests consume the same backend-shaped history/recovery data. Only host-dependent platform/shortcut availability is normalized for the fixture.

Shared `ipc-requests.json` values are emitted by rendered selection tests and decoded by actual Tauri command wrappers under its mock runtime. The probes check stale revision and malformed argument refusal before native clipboard initialization; they do not establish installed webview/native behavior. UI responders enumerate command response kinds and reject unknown commands instead of returning a successful snapshot for a typo.

After an intentional wire change, review both consumers and regenerate from the repository root in PowerShell:

```powershell
$env:SSH_CLIPBOARD_WRITE_IPC_FIXTURE = '1'
try { cargo test --locked --manifest-path desktop/src-tauri/Cargo.toml --all-features --bin ssh_clipboard_desktop_prototype ipc_contract_fixture_matches_backend_serialization }
finally { Remove-Item Env:SSH_CLIPBOARD_WRITE_IPC_FIXTURE }
```

Then run `npm test` and `npm run build` in `desktop/` and the desktop Rust checks. Normal CI never sets the write variable. Keep fixtures synthetic, deterministic and free of local configuration paths.

## Browser Contracts

From `desktop/`, install the lockfile-matched test browser once with `npx playwright install chromium`, then run `npm run test:browser`. An existing Chrome can be used without a browser download by setting `PLAYWRIGHT_CHANNEL=chrome` for the test command. The runner owns a headless isolated browser and a test-only Vite server on `127.0.0.1:1421`; it refuses to reuse an already-running server.

Three scenarios cover real Tab/arrow navigation and confirmation focus restoration, persistent Apply/Discard at the supported 340x440 minimum with long scrolling content, and browser constraint validation revealing a collapsed invalid port before any IPC save. Keep the wider state/race matrix in Vitest rather than duplicating it here. The suite uses the shared Rust-shaped responses and strict UI responder; test-only Vite aliases replace Tauri APIs, with no changes to normal dev/build/installed dispatch. External browser requests are blocked.

CI runs this credential-free Chromium lane whenever desktop checks are selected. Tests have bounded deadlines, one worker and no retries; failures retain screenshots, traces and the HTML report for seven days. Local reports are ignored under `desktop/artifacts/browser-*`. This proves browser layout and keyboard/form behavior with mock IPC, not actual native clipboard, shortcut registration, login or installer behavior. See [Playwright web-server configuration](https://playwright.dev/docs/test-webserver) and [test options](https://playwright.dev/docs/test-use-options).

## Transport and Diagnostic Faults

Pipe fixtures use noninteractive PowerShell or `/bin/sh`, with no SSH or clipboard. All transport fixture families share two-slot admission; keep timeout/uncertainty/reaping assertions and production deadlines intact. Linux process cases serialize cases to prevent fork-inherited executable writers causing ETXTBSY, while retaining concurrent clients inside each case. Fixture runtimes cap Tokio workers at two. See [session guardrails](persistent-sessions.md#evidence-and-fixture-guardrails).

Native Linux probes use disposable sockets and synthetic data to cover session exchanges, duplicate listeners, EOF/held-open stdin cleanup, restart epochs, and CLI exports/mutations. They do not establish live SSH-key authorization or native clipboard behavior.

For an authorized disposable shelf SSH probe from Windows:

```powershell
./scripts/Test-LiveShelf.ps1 -Target user@host -Binary target/linux-wsl/debug/ssh_clipboard
```

The runner builds the test first and starts a private remote daemon with a 120-second fixture lifetime. It checks protected Share/retry, exact bytes, Watch, operation lookup and cleanup on one owned SSH child. It neither installs a service nor accesses the OS clipboard.

## Desktop Update Evidence

From `desktop/`, `npm run test:package` builds and verifies an isolated synthetic-key Windows installer without installing it. On Mac, `npm run test:package -- --release` inspects the archived app/CLI and signatures without launching the GUI. Private fixture keys are removed; synthetic package evidence cannot qualify a production release.

Installed updater checks require the opt-in [native rehearsal](desktop-updates.md#isolated-native-updater-rehearsal). Prior [Windows results](history/desktop-update-acceptance.md) and [Mac results](history/macos-updater-acceptance.md) preserve their own source, trust, settings and cleanup boundaries. Framework mocks, package inspection, native replacement and anonymous public delivery are distinct evidence.

## Server Maintenance Evidence

The disposable server maintenance CI job runs both commands below on native Ubuntu when shared Rust, server smoke/build helpers, signing dependencies or unknown inputs change. The maintenance runner also retains Windows-to-WSL support. CI uses newly generated test trust and temporary homes with fake systemctl; it never opts into SSH mode, installs an actual user service or uses production signing secrets. Keep rollback and recovery in this lane rather than relying on a green package-verifier unit test.

From `desktop/`, `npm run test:server-package` verifies actual staging with disposable trust. `npm run test:server-maintenance` uses fake systemctl and real daemon/proxy processes in isolated Linux state. Unit grammar is checked by real `systemd-analyze --user verify`; this does not establish real service lifecycle.

The explicit `--ssh-target <authorized-host>` mode uses a runtime-only real user service, bounded signed debug fixtures and crash-loop rollback. It must preserve the installed service and actual SSH route. [Server maintenance](server-maintenance.md#real-user-systemd-acceptance) owns the fixture contract; [Linux cutover acceptance](history/linux-cutover-acceptance.md) separately records the exact installed production artifact. Actual reboot/last-session logout remain unobserved.

## Native Test Scheduling

The agreed Windows/Apple Silicon/Linux-server delivery and its broad manual campaigns are closed. Linux desktop and omitted rare scenarios are deferred and non-blocking; see the [closed scope](history/implementation-plan.md#future-work-and-verification-budget). Use existing automated/process/artifact checks first. A new native pass needs a concrete changed behavior or observed defect, a short finite checklist and one action at a time.

Browser checks establish actual layout, keyboard reachability and focus with mock IPC; rendered component tests alone cannot prove layout. Neither establishes native clipboard paste, global registration, actual login or installed replacement. Preserve unobserved cases explicitly. Earlier counts and accepted native results remain in [testing checkpoints](history/testing-checkpoints.md), [Windows history acceptance](history/native-history-acceptance.md) and [UX delivery evidence](history/desktop-ux-plan.md).

## Platform `cfg` Hygiene

Keep helpers and imports gated consistently with their callers. Use `#[cfg(any(target_os = "windows", test))]` only where tests need a platform helper, and mark deliberate platform-specific unused values in the matching branch. Windows-only checks cannot establish Linux/macOS warnings-as-errors correctness; run the affected platform variant or report that gap.

## Documentation Checks

Run `node scripts/check-docs.mjs` from the repository root to check tracked Markdown links/headings and current guide index coverage. CI runs this lightweight check even when compilation is skipped. Change selection is fail-closed and has regressions in `scripts/ci-changes.test.mjs`.

Frontend checks can be selected as `npm run test:tooling` (platform-sensitive Node helpers) or `npm run test:ui` (pure rendered interactions). Local `npm test` still runs both.

For documentation-only work, validate relative file and heading links, index coverage, referenced assets and `git diff --check`. Inspect `git status`/diff paths to ensure source, manifests, workflows and lockfiles are unchanged. Do not build packages or start live fixtures merely to validate prose. When moving evidence, preserve package hashes, source identities, timestamps and screenshots, and update inbound links.

## File Transfer Evidence

The ignored `clipboard::tests::native_file_clipboard_probe` is an explicit native acceptance tool, never part of ordinary CI clipboard access. Point `SSH_CLIPBOARD_NATIVE_FILES_ROOT` at a disposable directory with an `owner` file containing `ssh-clipboard-native-file-fixture-v1` plus a newline. Set `SSH_CLIPBOARD_NATIVE_FILES_MODE` to `capture` (one selected `source` folder to `outbound.scbf`), `publish` (`inbound.scbf` to the private fixture cache/native clipboard), `inspect` (fresh-process cache/reference check), or `recapture` (only the single root inside that same cache to `recaptured.scbf`). Run the filtered desktop test with `-- --ignored --nocapture` only with explicit native clipboard authority. It never loads production settings or contacts the server; ordinary CLI/process tests cover transport separately. Preserve published fixture paths until they are no longer referenced. [Recorded source acceptance](history/file-transfer-source-acceptance.md) distinguishes actual Explorer/Finder paste from native adapter checks and unobserved desktop UI behavior.

Tests cover bundle/tree validation, binary bytes, no-follow capture, portable renames, no-replace publication, session/CLI process round trips, cache quota/pins/restart retention, and partial/failed fake native publication. IPC fixtures include visible and hidden file names. Native Explorer/Finder acceptance remains separate: all Windows/Mac directions, hashes/trees, shortcuts, collisions and paste after hide/Quit/reopen. See [file transfers](file-transfers.md#compatibility-and-acceptance).

## Update Triggers

- Test commands, build variants, fixture isolation or observable contracts change.
- A failure exposes a missing guardrail or new evidence changes a recorded boundary.

## Related Docs

- [Build variants](build-variants.md), [CI](ci.md), [desktop development](desktop-development.md).
- [Connection diagnostics](connection-diagnostics.md), [release operations](releasing.md), [writing docs](writing-docs.md).
