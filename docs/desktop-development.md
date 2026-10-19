# Desktop Development

## Purpose

Build and validate the Tauri 2/Svelte desktop from source. Windows x64 and Apple Silicon are the shipped platforms; Linux desktop is deferred. The portable build keeps the historical Prototype name and identifier for development, while installed builds use the production identity.

## Key Files

- `desktop/package.json`, `desktop/package-lock.json`: frontend commands and dependency lock.
- `desktop/src/`: panel, settings, presentation and typed IPC.
- `desktop/src-tauri/`: separate Rust project, lockfile, native adapters and application commands.
- `desktop/scripts/build-prototype.mjs`: standalone executable and embedded-asset check.
- `desktop/scripts/build-installed.mjs`: signed installed-package staging.
- `desktop/src-tauri/tests/embedded_frontend.rs`, `live_ssh.rs`: standalone assets and opt-in V2 SSH probes.

## Run and Build

Use Node.js 24, Rust and the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). From `desktop/`:

```powershell
npm ci
npm run tauri dev
```

For a standalone development executable with embedded assets, without an installer:

```powershell
npm run build:prototype
```

Launch the timestamped `desktop/artifacts/SSHClipboardPrototype-<build-time>.exe` printed on Windows. The helper builds through Tauri with `custom-protocol`, checks embedded assets, then copies the executable outside mutable Cargo output. Closing the panel hides it; use Quit before replacing or leaving a test instance. A second launch can activate an already running instance.

Do not distribute `desktop/src-tauri/target/debug/ssh_clipboard_desktop_prototype.exe` directly for testing. A subsequent plain Cargo test can rebuild it without embedded frontend loading, causing `127.0.0.1 refused to connect`. The helper's separate timestamped copy prevents that overwrite.

The portable build cannot register startup or use the production updater. It does use the normal shared settings location unless explicitly isolated; a different app identifier alone does not isolate settings. Use the established [native updater fixtures](desktop-updates.md#isolated-native-updater-rehearsal) when installed identity, test trust or a separate settings store is needed. Signed package staging is documented in [desktop updates](desktop-updates.md#package-staging).

Mac helpers use the same minimum deployment target declared in Tauri configuration for both Tauri CLI and plain Cargo commands. Keep these environments aligned when changing the minimum supported version; otherwise native dependencies compile again when the standalone build switches to its embedded-asset test. The test and real portable executable build remain separate required checks.

CI may reuse its earlier checked frontend build with a validated generated proof; see [CI](ci.md#ci-overview). Ordinary local commands remain self-contained. Opt-in reuse rejects changed source, configuration, dependencies, backend-produced frontend fixtures, environment or generated assets rather than silently packaging stale frontend code.

## Runtime and Feature Ownership

The desktop links the root library without the old agent or system-clipboard feature. One native worker owns the clipboard adapter for the process lifetime and initializes it only for an explicit transfer. IPC exposes bounded application commands rather than general shell, filesystem or clipboard plugins.

- [Shared configuration](shared-configuration.md) owns drafts, profiles, coordinated Apply and live reload; [migration](configuration-migration.md) owns import and exact backups.
- [History](desktop-history.md) owns endpoint selection, preview privacy, selected-item Copy and confirmed mutations. Legacy panel Copy uses its displayed snapshot; shelf Copy revalidates the selected ID and epoch.
- [Persistent sessions](persistent-sessions.md) and [share recovery](share-recovery.md) own metadata monitoring, cancellation and uncertain uploads. Automatic metadata connections do not sample or overwrite the clipboard.
- [Shortcuts](desktop-shortcuts.md) and [notifications](desktop-notifications.md) own background actions, recording, feedback and keyboard behavior.
- [Desktop updates](desktop-updates.md) owns installed identity, startup and verified package handoff.

## Layout and Resource Guardrails

Keep header, settings navigation, save actions and footer outside the scrolling content. Bound both the grid row and column with `minmax(0, 1fr)`; hidden document overflow alone can conceal a widened column. Check actual content/header bounds, long targets, both themes and keyboard reachability at 440 x 670 and the minimum 340 x 440 CSS viewport. Rendered component tests cannot establish layout or native scaling.

Port and Identity file live in Advanced. Native invalid-field handling must open the containing disclosure before focus; known backend field failures must return to the section without dropping the draft. [UX delivery evidence](history/desktop-ux-plan.md) retains screenshots, measurements and the original review criteria.

On Windows, the desktop opts into `external-resource-owner`. Tauri owns executable resources; the root library must not embed another VERSION resource. Duplicate ownership caused `CVT1100`/`LNK1123`. Ordinary CLI/agent builds retain their own resource behavior.

## Validation

Use the frontend, desktop Rust and shared-library commands in [testing](testing.md#how-to-run). Test both dependency graphs: a compatible codec/parser version range in separate lockfiles is not proof that wire or migration fixtures remain compatible. Run root tests from the root project when they need root dev-dependencies.

For a targeted V2 SSH probe, provide an authorized `SSH_CLIPBOARD_TEST_TARGET` and an initially empty `SSH_CLIPBOARD_TEST_SOCKET` in a fresh owner-only remote directory, then run from the repository root:

```powershell
cargo test --manifest-path desktop/src-tauri/Cargo.toml --test live_ssh -- --ignored
```

The probe only writes synthetic data to the explicitly supplied disposable socket and sends PeekMeta to the regular route. It does not use the OS clipboard. Stop/remove the owned fixture afterward. Shelf SSH probes and native-test scheduling are documented in [testing](testing.md#transport-and-diagnostic-faults).

The [prototype snapshot](history/desktop-prototype.md#validation-status), [Windows history acceptance](history/native-history-acceptance.md), [UX delivery record](history/desktop-ux-plan.md) and [release evidence](history/release-readiness.md) retain exact build/session boundaries. Those results do not transfer automatically to a later source build.

## Update Triggers

- Development commands, artifact paths, build identities or platform requirements change.
- Native ownership, resource embedding, IPC, layout or validation boundaries change.

## Related Docs

- [Architecture](../ARCHITECTURE.md), [build variants](build-variants.md), [CI](ci.md), [testing](testing.md).
- [Dependency decisions](dependency-decisions.md), [release operations](releasing.md), [docs index](index.md).
