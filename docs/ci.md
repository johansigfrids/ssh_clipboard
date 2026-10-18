# CI

## Purpose
Document the GitHub Actions setup for tests and releases, including build variants and required system packages.

## Key Files
- `.github/workflows/ci.yml` (PR/push checks)
- `.github/workflows/desktop-release.yml` (manual environment-gated Windows and Apple Silicon signed drafts)
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

On 2026-10-16, a full Windows run exceeded three existing elapsed-time assertions while separately admitted transport modules competed with each other and compilation. The isolated timeout probes passed. Legacy, one-shot session and persistent-session pipe fixtures now acquire the same two-slot test semaphore before spawning, instead of independent module limits. Full Windows and headless WSL suites subsequently passed with their timing/uncertainty/reaping assertions unchanged. Keep admission shared across fixture families; do not hide startup contention by extending production deadlines or dropping cleanup assertions.

## Desktop Prototype Checks

The `desktop` job checks the isolated Phase 1 prototype on Windows, macOS, and Linux. It uses Node.js 24, the committed npm/Cargo lockfiles, Svelte type/build checks, rendered frontend interaction tests, Rust formatting/lint/tests, and `npm run build:prototype` for a standalone executable with an embedded-asset check. Linux installs WebKitGTK 4.1 and tray/clipboard build dependencies. This job does not publish artifacts or run a native GUI session; OS clipboard/paste and focus/shortcut behavior need separate evidence in [desktop-prototype.md](desktop-prototype.md).

## Retired Legacy Publisher

The old automatic `v*` agent/server publisher (`release.yml`) is removed as part of [legacy retirement](legacy-retirement.md). Historical v0.3.0 archives/tags are preserved. Current distribution uses the protected desktop/server draft workflows below; creating a new `v*` tag on current source does not publish an old-agent build.

The existing compatibility matrix remains automated regression coverage. Its agent runners also check the default dependency tree for absence of old agent/tray/startup libraries and execute the default CLI command-surface regression. Existing Linux desktop compilation is not a new native Linux acceptance requirement.

## Desktop Release Staging

Desktop CI also lints/tests installed features without release credentials and builds a synthetic-key NSIS package on Windows without installing it. The mock framework handoff and bounded download probes run as tests. The separate manual desktop workflow pins actions/toolchain, validates an immutable master commit, and exposes signing credentials only in its protected packaging step. It creates drafts; the owner has configured environment protections/signing inputs, and protected signing/publication and accepted native paths have since passed for the shipped platforms. See [release operations](releasing.md).

The macOS PR job now also builds a release-profile test app with disposable updater trust and an explicit ad-hoc code signature, then checks the extracted bundle identity/version, app/CLI architecture, code signatures and CLI version without starting the GUI. The public package/inspection evidence is retained for seven days after private test-key cleanup. The first probe and all seven jobs passed in [run 38919474529](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38919474529) for PR head `4a05401` (synthetic merge `3c8897a`). A later private actual-Mac CLI/import probe also passed; see [Mac evidence](macos-cutover.md#status). This does not establish GUI installation, paste, startup or updater replacement, and subsequent candidate changes need fresh exact-head CI.

Protected desktop staging validates Windows/macOS without credentials, then builds on both native runners behind the same reviewer environment. Only the final draft job receives release-write permission; it downloads artifacts from its own workflow run and invokes `desktop/scripts/desktop-draft.mjs`. Both platforms must match immutable source, production identity and the independently pinned public trust. One shared keyed feed contains both admitted platform entries. A partial-platform, inconsistent-notes/key/version or existing-tag result cannot create a draft. The expanded workflow passed for desktop 0.4.2; see [publication acceptance](release-0.4.2-acceptance.md).

Frontend Node tests now also execute the desktop draft handoff with inert filesystem/GitHub adapters, rejecting wrong/dirty source, debug/test identity, wrong CLI/platform/version, changed bytes/checksums, missing assets, ambiguous stages and existing releases. A temporary Git repository checks clean/dirty/staged/untracked/source-change admission. The Windows synthetic installer checks its public evidence against the exact newly verified package and removes its disposable private key. See [release readiness](release-readiness.md).

## Server Release Staging

The manual server workflow validates the exact stable core commit on master, pins Rust/actions, and runs headless checks without credentials. A separate `server-release` environment gates the Linux x64 signing step. It builds on Ubuntu 24.04, verifies exact signed bytes/version/platform with the server policy, and creates a `server-v<version>` draft without publishing/overwriting or becoming latest. The credential-free validation job also executes the actual draft handoff with inert adapters, checking source/platform/profile refusal, no overwrite and desktop-feed isolation. Source SHA/profile/key ID/checksum evidence accompanies the package. The production 0.4.0 workflow, independent signatures, isolated real user-systemd runtime and clean live cutover have passed; see [server acceptance](linux-cutover-acceptance.md). Other architectures are outside the delivered scope and actual live reboot/logout remain unobserved. See [release setup](releasing.md#signed-server-drafts).

## Released Compatibility Baseline

After its optimized headless build, the Linux server job runs the seven standard-library Python guard tests and [released-client compatibility probe](released-compatibility.md). The latter downloads the fixed v0.3.0 archive, checks its reviewed SHA-256 before execution and exercises old/new CLI/proxy/daemon routes using private sockets and synthetic data. It never starts SSH, systemd or an OS clipboard adapter. A pinned upload-artifact action retains the metadata-only pass/failure/cleanup report for 14 days, including on a failed probe. Reports live in the job's fresh runner temporary directory, outside cached Cargo output, so a restored cache cannot collide with the no-overwrite evidence contract. Other platform jobs do not execute this Linux baseline. A network/download failure fails the check; it is not reported as successful compatibility.

## Update Triggers
- Changing build variants, features, or supported OSes.
- Adding/removing system package dependencies.
- Modifying artifact naming or release packaging steps.

## Related Docs
- `docs/testing.md`
