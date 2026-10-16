# macOS Clean Cutover

## Purpose

Replace the owner's legacy 0.3 Mac client with one installed 0.4 desktop client. Completion includes settings migration, connection to the managed history server, working login startup, and removal of obsolete active binaries, startup entries and configuration. A second permanently installed test client is not the target.

## Key Files

- `desktop/scripts/build-installed.mjs`: native application/updater package and independent verification.
- `desktop/scripts/smoke-installed-build.mjs`: disposable signing identity and package admission.
- `.github/workflows/ci.yml`, `.github/workflows/desktop-release.yml`: platform checks and protected production signing.
- `desktop/src-tauri/src/startup_native.rs`, `desktop/src-tauri/src/panel.rs`: installed startup and desktop lifecycle.
- `src/configuration.rs`, `src/configuration/migration.rs`: guarded legacy import and exact backup.

## Scope and Initial Inspection

The owner requested a full replacement on 2026-10-16. Read-only SSH inspection found an Apple Silicon Mac running macOS 27.0.1, with Xcode 27.1 and Rust 1.96.0. Node/npm were not available in the inspected SSH PATH or common Homebrew/version-manager locations. Build tools can stay on CI; the installed client must not require Node or Cargo.

No clipboard client process or application bundle was found in the inspected process list and standard application directories. A loaded `~/Library/LaunchAgents/ssh_clipboard.plist` still requests `/usr/local/bin/ssh_clipboard agent`, but that executable is absent and launchd reports spawn failure. The legacy `~/.config/ssh_clipboard/agent.toml` remains, with saved direct shortcuts and a startup preference. Its server name is obsolete. These are inspection results, not proof that every possible installation location has been inventoried.

The new server's current hostname failed SSH host verification from the Mac. Resolve that route using an independently verified server host key and the Mac's own authentication. Never use `StrictHostKeyChecking=no`, copy private SSH keys, or infer GUI-session authentication from a different machine's successful connection.

## Delivery Sequence

1. **Prove the application package automatically.** Build an Apple Silicon `.app.tar.gz` using installed features and a disposable updater signing key. Inspect the archived application identity/version, native executable and matching clipboard-only CLI, and verify macOS code signatures. This does not install or start the app, prove native paste, or admit a production release.
2. **Prepare protected production packaging.** Use an unused 0.4 desktop version after the package proof passes. Preserve published 0.4.1 assets and the Windows update route. A shared latest feed must retain admitted entries for every delivered platform; a Mac-only feed must not displace Windows updates. Production updater signing stays in the protected GitHub environment, with independent verification before installation.
3. **Prepare the real cutover.** Finish the actual Mac-to-server SSH trust/authentication check. Inventory exact legacy paths and startup ownership, and make a private recovery backup outside active application/configuration locations. Import existing settings without losing direct shortcut preferences; explicitly select the current server's history endpoint. No startup registration is inferred from an imported preference.
4. **Install and accept the new client.** Install the verified application in a stable user-owned location, enable its own login startup, and check the actual registered executable and `--background` argument. Use the owner's ordinary GUI session for one short native transfer/paste check, shortcut/panel check and login check. Automate package, protocol and metadata checks; guide unavoidable manual actions one at a time. Do not repeat the completed Windows test campaign.
5. **Retire the old client.** After the new client works, unload and remove the exact legacy LaunchAgent, remove verified obsolete executable links/files, and move the old active configuration/logs out of active discovery paths. Preserve one clearly identified private recovery backup, SSH keys and unrelated applications. Remove any temporary test app/startup entry created by this work. Verify that only the installed 0.4 client owns clipboard startup and shortcuts.
6. **Close the evidence.** Record exact source/package/signature identity, automated checks, native observations, cleanup inventory and any remaining update/Gatekeeper gaps. Publication remains a separate reviewed action; no planned check counts as a passed check.

## Signing and Update Boundaries

The existing free updater key authenticates package bytes and release identity. It does not provide Apple Developer ID or notarization. Start with the framework's supported ad-hoc macOS signature (`signingIdentity: "-"`) for the free route; it requires no Apple private signing identity. A downloaded app may still require the owner's explicit approval in macOS Privacy & Security. Do not disable Gatekeeper or remove quarantine globally. See [Tauri's signing guidance](https://v2.tauri.app/distribute/sign/macos/#ad-hoc-signing).

Native update acceptance is separate from static package signature verification. Prove an actual replacement/restart on macOS before claiming that the installed Mac updater is accepted. Do not use a disposable test key as production trust or copy the production private key to the Mac.

## Status

Inspection and the clean-cutover scope are recorded. No Mac installation, startup change, settings migration, SSH trust change or cleanup has taken place. macOS package/native/update acceptance remains open.

## Update Triggers

- Mac package, startup, signing or updater behavior changes.
- Native installation or actual login/paste/update results become available.
- Legacy inventory, backup or cleanup results change.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md)
- [Compatibility and cutover](compatibility-and-cutover.md), [configuration migration](configuration-migration.md)
- [Desktop updates](desktop-updates.md), [release operations](releasing.md), [security](security.md)
