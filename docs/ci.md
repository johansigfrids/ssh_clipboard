# CI

## Purpose
Document the GitHub Actions setup for tests and releases, including build variants and required system packages.

## Key Files
- `.github/workflows/ci.yml` (PR/push checks)
- `.github/workflows/desktop-release.yml` (manual environment-gated Windows and Apple Silicon signed drafts)
- `.github/workflows/server-release.yml` (manual environment-gated Linux x64 signed drafts)
- [docs/testing.md](testing.md) (local test commands and Linux deps)

## CI Overview

CI always checks tracked Markdown links/headings, current guide index coverage and change-selector regressions. Markdown and documentation images under `docs/images/` (`png`, `jpg`, `jpeg`, `gif`, `webp`, `svg`, case-insensitive) skip compilation. Screenshots accompanying frontend changes therefore retain the desktop/browser checks without selecting core, maintenance or packages. Keep this allowlist narrow: public keys, scripts and unfamiliar documentation inputs still select all checks. Renames include both paths, so moving an image out of the documentation image directory also selects its new location's checks.

Unknown paths select all checks; shared Rust changes select core and desktop, frontend source/tests select desktop, and desktop build/configuration/dependency/tooling changes also select synthetic packages. Manual CI dispatch runs every check, including packages, before a release when needed.

Superseded runs for the same PR are cancelled. Each master push and manual dispatch uses its own concurrency group, so Actions can schedule them independently (subject to account capacity). Do not group master runs by branch with cancellation disabled: Actions keeps only one running and one pending run per group, so a third push replaces the pending run even though cancellation is disabled. Distinct run IDs prevent both that loss and unnecessary branch-level serialization. The always-run `CI complete` check fails if selection or any selected job fails or is cancelled. Use this stable check for future branch protection rather than a conditionally skipped matrix entry.

Cargo caches use the pinned Rust Cache action, which keys by actual toolchain/environment and prunes workspace binaries and incremental output. Core platforms have separate cache keys.

Package-selected Windows/Mac jobs separately cache the bundled CLI's dependencies in `target/desktop-sidecar`. The installed-build helper selects that directory only when `SSH_CLIPBOARD_CLI_TARGET_DIR` is provided; ordinary local builds retain root `target/`. Keep the exact headless CLI build and CLI/archive verification: the packaged CLI has the same session-only contract as the root build. The desktop cache alone does not cover the root CLI workspace, which previously rebuilt all of those dependencies on each package run.

On Mac, set `MACOSX_DEPLOYMENT_TARGET` from the checked-in Tauri configuration before restoring caches or invoking Cargo. Build helpers use that same value. Tauri CLI sets it automatically, whereas plain Cargo inherits the shell; switching between the two environments rebuilds native dependencies. The explicit `10.13` configuration preserves Tauri's previous default, and the value participates in desktop/sidecar cache keys.

The frontend check/build runs once per desktop job and records a generated proof. Only the packaging and standalone steps opt into `SSH_CLIPBOARD_PREBUILT_FRONTEND=1`: the helpers verify source/configuration/dependency/fixture hashes, Node version, `NODE_ENV`/`VITE_*` inputs and every generated asset before disabling Tauri's repeated frontend hook. Missing/stale proof or changed output fails the opt-in build. Current frontend output is independent of Tauri's debug/installed/native build flags; retain that contract or introduce separate proofs before adding platform-specific frontend output. Normal local/release commands keep the complete frontend build hook. Never remove Svelte checks or the standalone Wry asset test to save compilation time.

Platform-sensitive Node tooling tests run on all desktop hosts. Pure Svelte interactions run once on Linux. Default-feature desktop Rust is linted everywhere and tested on Linux; all-feature tests and standalone embedded-asset builds run on every desktop host. Keep default lint and Linux tests: installed features exclude portable-only code.
- `ci.yml` runs on PRs and pushes to `master`:
  - `cargo fmt -- --check`
  - `cargo clippy --locked --all-targets -- -D warnings`
  - `cargo test --locked`
  - `cargo build --locked --release`
- Core matrix: Windows, macOS and Linux, each using the same headless commands.

The server job installs no GUI prerequisites and rejects OS clipboard/tray/hotkey/X11/Wayland/GTK dependencies in its normal Cargo tree before building. The desktop owns native clipboard access; the root CLI is always headless; see [build variants](build-variants.md).

Linux-only maintenance code must pass warnings-as-errors on the CI toolchain as well as Windows checks. Use `Err(eyre!(...))` for match-result expressions, or a statement block with `bail!(...);`: the pinned eyre macro emits a trailing semicolon that Rust rejects in expression position. Older local Linux toolchains may miss this diagnostic.

Process-backed timeout fixtures must allow Windows PowerShell to start and drain their synthetic stderr before the operation assertion. The persistent-connection uncertainty probe uses the shared ten-second fixture budget rather than three seconds; it still requires an unacknowledged mutation timeout, retained epoch and reaped child. Production timeout defaults are unchanged.

All pipe-fixture families share two-slot admission before spawning. Keep it shared across modules so Windows startup contention does not invalidate timeout/uncertainty/reaping assertions. The [dated checkpoint](history/testing-checkpoints.md#ci-packaging) records the reproduced contention and fix; production deadlines remain unchanged.

## Desktop Checks

The independent `Browser panel contracts` job runs three Chromium scenarios for real keyboard navigation, minimum-size layout and native HTML form validation when desktop checks are selected. It needs no Rust/desktop GUI dependencies or production credentials. Browser version follows the committed Playwright package; CI installs Chromium explicitly. Failures retain traces/screenshots/report for seven days and fail `CI complete`. The test-only Vite aliases and synthetic IPC do not enter production builds.

The `desktop` job checks the desktop source and portable development build on Windows, macOS, and Linux. It uses Node.js 24, the committed npm/Cargo lockfiles, Svelte type/build checks, rendered frontend interaction tests, Rust formatting/lint/tests, and `npm run build:prototype` for a standalone executable with an embedded-asset check. Linux installs WebKitGTK 4.1 and tray/clipboard build dependencies. This job does not publish artifacts or run a native GUI session; OS clipboard/paste and focus/shortcut behavior need separate evidence in [desktop development](desktop-development.md).

## Retired Legacy Publisher

Only the protected desktop and server release workflows remain. Historical tags/assets are preserved. No agent build or release workflow exists.

## Desktop Release Staging

Desktop CI also lints/tests installed features without release credentials and builds a synthetic-key NSIS package on Windows without installing it. The mock framework handoff and bounded download probes run as tests. The separate manual desktop workflow pins actions/toolchain, validates an immutable master commit, and exposes signing credentials only in its protected packaging step. It creates drafts; the owner has configured environment protections/signing inputs, and protected signing/publication and accepted native paths have since passed for the shipped platforms. See [release operations](releasing.md).

The separate Mac updater rehearsal derives its baseline from the committed stable core version and stages that version plus the next patch. Its temporary lockfile rewrites use the same baseline and restore the original files afterward. Do not pin this helper to a past production version: that made an otherwise valid schema-only change fail at the next release bump.

The macOS PR job builds a release-profile test app with disposable updater trust and ad-hoc code signing, then checks bundle identity/version, app/CLI architecture, signatures and CLI version without starting the GUI. Public inspection evidence is retained for seven days after private-key cleanup. The [packaging checkpoint](history/testing-checkpoints.md#ci-packaging) and [Mac cutover](history/macos-cutover.md) record historical exact-source results separately.

Protected desktop staging validates Windows/macOS without credentials, then builds on both native runners behind the same reviewer environment. Only the final draft job receives release-write permission; it downloads artifacts from its own workflow run and invokes `desktop/scripts/desktop-draft.mjs`. Both platforms must match immutable source, production identity and the independently pinned public trust. One shared keyed feed contains both admitted platform entries. A partial-platform, inconsistent-notes/key/version or existing-tag result cannot create a draft. The expanded workflow passed for desktop 0.4.2; see [publication acceptance](history/release-0.4.2-acceptance.md).

Frontend Node tests now also execute the desktop draft handoff with inert filesystem/GitHub adapters, rejecting wrong/dirty source, debug/test identity, wrong CLI/platform/version, changed bytes/checksums, missing assets, ambiguous stages and existing releases. A temporary Git repository checks clean/dirty/staged/untracked/source-change admission. The Windows synthetic installer checks its public evidence against the exact newly verified package and removes its disposable private key. See [release readiness](history/release-readiness.md).

## Server Release Staging

The separate PR/push `Disposable server maintenance` job exercises synthetic signed server staging, wrong-key rejection, real daemon/proxy health, failed initial enable/restart cleanup, update rollback and interrupted-journal recovery. It runs on native Ubuntu with a 25-minute job limit, temporary homes, fake systemctl and disposable trust. The selector includes this lane for shared Rust/server tooling/dependency changes and conservatively for unknown inputs; desktop-only source changes skip it. Its result participates in `CI complete`. Actual installed service lifecycle remains separate evidence.

The manual server workflow validates the exact stable core commit on master, pins Rust/actions, and runs headless checks without credentials. A separate `server-release` environment gates the Linux x64 signing step. It builds on Ubuntu 24.04, verifies exact signed bytes/version/platform with the server policy, and creates a `server-v<version>` draft without publishing/overwriting or becoming latest. The credential-free validation job also executes the actual draft handoff with inert adapters, checking source/platform/profile refusal, no overwrite and desktop-feed isolation. Source SHA/profile/key ID/checksum evidence accompanies the package. The production 0.4.0 workflow, independent signatures, isolated real user-systemd runtime and clean live cutover have passed; see [server acceptance](history/linux-cutover-acceptance.md). Other architectures are outside the delivered scope and actual live reboot/logout remain unobserved. See [release setup](releasing.md#signed-server-drafts).

## Released Compatibility Baseline

The released V2 binary download/probe and evidence artifact are removed in 0.5. Core CI runs one headless build on each Windows/macOS/Linux platform, retaining dependency guards and session process tests.

## Update Triggers
- Changing build variants, features, or supported OSes.
- Adding/removing system package dependencies.
- Modifying artifact naming or release packaging steps.

## Related Docs
- [docs/testing.md](testing.md)
