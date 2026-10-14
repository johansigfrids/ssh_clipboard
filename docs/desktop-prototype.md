# Desktop Foundation Prototype

## Purpose

Document the Phase 1 Tauri/Svelte candidate, how to run it, and which platform contracts have been verified. This is an isolated prototype, not the shipping replacement for the agent.

## Key Files

- `desktop/package.json`, `desktop/package-lock.json`: frontend commands and pinned dependency tree.
- `desktop/scripts/build-prototype.mjs`: standalone build and protected copy outside Cargo's build output.
- `desktop/src/App.svelte`, `desktop/src/api.ts`: panel, settings, and native command boundary.
- `desktop/src/shortcut.js`: physical-key shortcut capture.
- `desktop/src-tauri/Cargo.toml`, `desktop/src-tauri/Cargo.lock`: separate Rust project and dependency graph.
- `desktop/src-tauri/src/panel.rs`: session state, transfer commands, tray, shortcuts, and lifecycle.
- `desktop/src-tauri/src/clipboard.rs`: retained native clipboard worker.
- `desktop/src-tauri/src/model.rs`: settings validation and bounded previews.
- `desktop/tests/`: recorder and rendered Svelte interaction tests.
- `desktop/src-tauri/tests/live_ssh.rs`: opt-in real SSH probe using a disposable server socket.
- `desktop/src-tauri/tests/embedded_frontend.rs`: generated standalone-context check for the entry page and referenced assets.
- `build.rs`, `Cargo.toml`: opt-in resource ownership for a desktop host.

## Run and Build

Use Node.js 24 and Rust with the [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/). Commands below run from `desktop/`:

```powershell
npm ci
npm run tauri dev
```

For a standalone development executable with embedded frontend assets, without an installer:

```powershell
npm run build:prototype
```

On Windows launch the timestamped `desktop/artifacts/SSHClipboardPrototype-<build-time>.exe` printed by the helper. The helper builds through Tauri with embedded assets, checks the generated asset context, then copies the executable outside Cargo's mutable output. Each build gets a new name so a previous running test copy does not prevent a new build. Ordinary Rust checks are still useful for source validation.

Do not hand out `src-tauri/target/debug/ssh_clipboard_desktop_prototype.exe` for manual testing. A later plain Cargo integration test can rebuild that same path without `custom-protocol`, changing it into a development executable that requires the local Vite server. This caused the native 2026-10-13 test to display `127.0.0.1 refused to connect`. The separate artifact path avoids that overwrite; debug optimization does not mean the executable should depend on a dev server. Quit the old instance through its tray menu before launching a rebuilt artifact: closing only hides it, and single-instance activation otherwise focuses the old process.

The prototype has its own application identifier and lockfiles. It links the shared library without the old agent or system clipboard features and owns its retained native adapter. Settings use a separate versioned user file; see [shared configuration](shared-configuration.md). Startup can initialize missing settings from a backed-up legacy agent file, and the first save upgrades schema 1 with an exact backup; see [migration/recovery](configuration-migration.md). It does not install itself, change OS start-at-login, or replace either existing executable. Quit explicitly before leaving a native test session; closing the window only hides it.

## Controls and Data Flow

1. Open Settings and enter an existing SSH alias or `user@host`. Port and identity-file reference are optional. Apply the draft; settings are saved on this computer and restored after Quit/relaunch. CLI edits reload automatically between operations. Corrupt settings produce a recovery message and block saving over the original file; repaired files are retried automatically.
2. Test saved connection sends `PeekMeta` through the existing proxy, so it also works with a proxy-only forced command. SSH uses batch mode; unlock/trust keys and hosts through your existing SSH setup before using the panel.
3. Refresh shared clipboard retrieves the current server value and prepares a preview without writing the local clipboard. A successful empty reply explicitly says the server clipboard is empty; it is distinct from not having checked yet. Connection changes reset that knowledge, and failed refreshes never establish an empty state.
4. Share current clipboard captures text or PNG through the existing client helpers and uploads it once. Successful sharing also makes that captured value the displayed snapshot.
5. Copy to this computer writes that displayed snapshot, even if another machine has since changed the server value. It never silently refreshes and copies a different item.

Opening the app/panel does not read the local clipboard, download clipboard content, or transfer it. Native clipboard access is initialized lazily on the first explicit read/write. The worker serializes operations and retains its adapter after Copy or window dismissal, then drops it on its owning thread at Quit.

The default panel shortcut is `CommandOrControl+Shift+V`; its recorder uses physical key codes. Settings show actual registration failure instead of silently ignoring it. Changing the shortcut registers the replacement before releasing the old one; failure preserves active settings. An empty connection is valid for changing UI preferences, but transfer/test actions require a target.

Tray menu actions open the panel/settings or quit. A second launch focuses the existing instance. The panel also has Quit, so exiting does not depend on tray support. On Wayland the X11 shortcut plugin is not initialized; launcher/menu access remains available. GlobalShortcuts portal integration is still pending.

Connection settings support named profiles. Choose a saved connection and Apply to make it active; Add starts an independent blank connection. Renaming preserves its identity and cached item. Removing requires confirmation, keeps at least one profile, and selects the first remaining connection. Switching/removing a profile clears its cached item even when the next profile uses the same target. None of these actions transfers clipboard contents. See [connection contracts](shared-configuration.md#desktop-connections).

The app shell is bounded to the WebView height: header and footer stay visible, and the middle content area scrolls when settings/previews need more room. Changing pages or settings sections starts at the top. Use a grid row with `minmax(0, 1fr)` and a scrollable content element rather than a growing `min-height` shell; the latter made Connection settings scroll the whole window and push Quit offscreen at Windows display scaling. Check real layout dimensions and keyboard reachability at the normal 440 x 670 and minimum 340 x 440 CSS viewport sizes after layout changes; component tests alone cannot detect overflow.

## Privacy and Limits

- Clipboard text is rendered as text, not HTML. Labels use the first non-empty line, capped at 60 characters; text previews cap at 2,000 characters. Display truncation does not alter copied bytes.
- Captured PNGs use balanced lossless compression and validate dimensions/pixel-buffer size before encoding. Images are validated with a 64 MiB decoded-pixel limit, then reduced to a maximum 320 x 180 thumbnail for display; copied pixels are not resized. Turning off previews withholds text labels/text and image thumbnails from the frontend.
- Switching target/port/key reference clears the previous connection's displayed snapshot. Settings and transfers share an operation lock; stale concurrent commands cannot activate a different connection mid-transfer.
- Transfers use version 2 one-shot transport and the selected profile's frame limit, defaulting to 10 MiB. Before SSH, Share validates the entire encoded request, including metadata, so a payload near the boundary cannot be mistaken for fitting. Capture/transfer errors report measured sizes and a next action. Changing a client limit does not change the remote proxy/daemon. Full memory accounting and structured SSH diagnostics are still rework items.
- Raw unexpected responses are not formatted into frontend errors. Error presentation is currently a concise generic connection explanation; use the CLI doctor for deeper diagnostics.
- The WebView has event-listener permissions and specific application commands, without shell/filesystem/clipboard plugins exposed directly to frontend code. A browser-only preview cannot invoke SSH or OS clipboard operations.

## Validation Status

Evidence recorded on 2026-10-13:

| Surface | Evidence | Remaining gap |
| --- | --- | --- |
| Existing Windows client | 57 unit, seven migration, and five executable-level settings tests pass, including wire fixtures, exact backups, advanced policy overrides, and profile removal; required agent lint/format pass. The headless variant passes 43 + 6 + 6 checks and all-target lint. | Native Linux build remains for CI; no new native legacy-agent session exercised. |
| Prototype backend | Windows compile/executable build, warnings-as-errors lint, 27 Rust tests, and the standalone embedded-asset check pass. Profile lifecycle probes use a real temporary store and include failure/rollback paths. Native Windows schema 2 save/relaunch and new RGBA adapter checks are user-confirmed. | Unit ownership/activation checks use controlled adapters; native profile controls and other platforms remain unverified. |
| Prototype frontend | Svelte type/build checks, 3 recorder tests, and 25 rendered component tests pass, including profile drafts/removal/recovery, empty versus unchecked/unreachable server state, rejected Share, settings recovery, reload/draft/race guards, and import summary. | Component tests mock the native command boundary. |
| Browser UI | Panel/settings navigation, Ctrl+Shift+K capture, and 440 x 670 / 340 x 440 layouts inspected. Document dimensions match the viewport; header/footer stay visible while settings scroll, and keyboard navigation reaches lower controls. | Browser cannot prove Tauri IPC, OS registration, focus, or native paste. |
| Native Windows | User confirmed all seven smoke checks on 2026-10-13: standalone launch, connection Apply/Test, scrolling with Done/Quit visible, small-image Share/Copy and paste into Paint after dismissal, shortcut reopening with keyboard focus, tray reopening, and second-launch activation of the existing instance. Earlier text sharing/pasting also passed. Subsequent persistent-settings and schema 2 builds passed Apply/Quit/relaunch restoration; the 2026-10-14 schema 2 artifact also passed small-image Share/Copy/paste into Paint. | User-confirmed evidence; live legacy TOML import, shortcut conflicts, multiple monitors, fault recovery, and other platforms still need their broader acceptance checks. |
| Real SSH | Installed Linux 0.3.0 service passes a metadata-only probe through the desktop dependency graph and current transport. A separate temporary daemon passes empty-state, exact Unicode/whitespace text, PNG bytes/pixels, metadata, retained snapshot, invalid-UTF-8 rejection, and missing-socket checks. | Probe selects the disposable socket directly; Tauri IPC and native clipboard transfers still need a desktop session. |
| macOS/Linux | CI jobs defined for source/build checks. | Jobs not executed here; no native X11/Wayland/macOS validation. |
| Installation/update | No installer or updater enabled. | Stable installation, signatures, upgrade/recovery, and key rotation remain later phases. |

Phase 1 is **not complete** until native checks pass on the supported platforms. Framework selection is still provisional.

The 10 MiB limit is inherited from the installed version 2 client/proxy/daemon, not an SSH limitation. [The rework plan](implementation-plan.md#decisions-to-prove-before-committing) now proposes larger configurable limits with capability negotiation and separate shelf/decoded-pixel budgets. Raising the desktop limit alone cannot fix rejection by an unchanged server. The current prototype remains compatible with the installed server; it does not change its service or restart its in-memory clipboard.

## Native Acceptance Checklist

Use a disposable SSH clipboard daemon/account and synthetic text/images for transfer checks; sharing must not overwrite a user's regular shared clipboard during validation.

- Launch normally, close/hide, reopen via shortcut/tray/launcher, and launch a second instance. Check focus, keyboard access, monitor placement/scaling, and that one instance remains.
- Test empty/unset server, exact whitespace/Unicode text, and PNG. Copy, dismiss the panel, and paste into another native application. Confirm clipboard ownership survives until explicit Quit where required.
- Change the server value from another client after Refresh; Copy must still write the displayed snapshot. Refresh explicitly to see the new value.
- Occupy the chosen shortcut, apply a replacement, and confirm registration failure preserves the active binding. Disable shortcuts and use alternate entry points.
- Change connection/port/key while an operation is pending, hide previews, and disconnect. Check clear feedback, serialization, and removal of old previews.
- Edit preview preferences or select another profile through the rework CLI while the panel is open. Confirm live reload after the active operation, preservation of an unsaved draft, Discard recovery, and rejection of outdated transfer clicks. The reload itself must perform no clipboard/SSH transfer.
- In Connection settings, add a temporary named profile, Apply, and Test. Rename it and relaunch to check persistence. Switch between profiles with Apply and confirm the old preview is cleared; while the temporary profile is active, remove it with confirmation and confirm the original returns. A shared target is sufficient for this isolation check; no second server is required.
- Exercise authentication rejection, untrusted host, missing proxy/daemon, and timeout. Check child-process cleanup and whether SSH produces an unwanted console window on Windows.
- On Linux, separately test X11, a Wayland session, missing tray, and launcher activation. Record the compositor and actual capabilities; environment detection is not proof of portal support.

## Automated Checks

From `desktop/`:

```powershell
npm run build
npm test
```

From the repository root:

```powershell
cargo fmt --manifest-path desktop/src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path desktop/src-tauri/Cargo.toml
cargo test --features agent
cargo clippy --features agent -- -D warnings
```

The prototype's codec fixture check is intentional: its separate lockfile can select a different compatible codec release from the root lockfile. Never assume a dependency version range proves wire compatibility. Root crate tests cannot be selected through the standalone prototype workspace when they require root dev-dependencies; run the root suite separately.

### Opt-in SSH Probe

Normal tests ignore `live_ssh.rs`; CI does not connect to a user's server. To run it, start the existing Linux binary's `daemon --socket-path` in a fresh, owner-only `/tmp/ssh-clipboard-prototype.<alphanumeric-token>/` directory. Set `SSH_CLIPBOARD_TEST_TARGET` to an authorized SSH target and `SSH_CLIPBOARD_TEST_SOCKET` to that directory's `daemon.sock`, then run:

```powershell
cargo test --manifest-path desktop/src-tauri/Cargo.toml --test live_ssh -- --ignored
```

The probe requires an initially empty disposable daemon. It performs only `PeekMeta` against the regular installed service, and writes synthetic values only to the supplied temporary socket. It never initializes the OS clipboard. Test paths and host references are environment variables, not committed machine configuration. Stop the temporary process and remove its socket/log/directory afterward; confirm the regular service is still active.

The 2026-10-13 run passed against the existing Linux 0.3.0 binary. The temporary daemon and directory were removed afterward. Socket selection uses a test-specific SSH invocation because the current shared transport fixes the remote command to `ssh_clipboard proxy`; this proves wire/server compatibility without claiming the desktop window was exercised.

## Windows Resource Guardrail

Linking the existing library into Tauri initially produced `CVT1100: duplicate resource` / `LNK1123`: both build scripts embedded a Windows VERSION resource. The prototype now opts into `external-resource-owner`, making the host the sole owner of executable resources. Normal CLI/agent builds retain their existing resource behavior. Future desktop hosts must use this feature rather than disabling all icon handling or patching linker flags around duplicate records.

## Update Triggers

- Native acceptance results or supported platform versions are recorded.
- The prototype becomes a shared workspace component or its settings start persisting.
- Clipboard ownership, shortcut/portal support, or resource embedding changes.
- New installer/updater functionality makes any limitation obsolete.

## Related Docs

- [Implementation plan](implementation-plan.md)
- [Architecture](../ARCHITECTURE.md)
- [Dependency decisions](dependency-decisions.md)
- [Protocol](protocol.md), [testing](testing.md), and [CI](ci.md)
- [Linux desktop](linux-client.md) and [agent](agent.md)
