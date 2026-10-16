# macOS Clean Cutover

## Purpose

Replace the owner's legacy 0.3 Mac client with one installed 0.4 desktop client. Completion includes settings migration, connection to the managed history server, working login startup, and removal of obsolete active binaries, startup entries and configuration. A second permanently installed test client is not the target.

## Key Files

- `desktop/scripts/build-installed.mjs`: native application/updater package and independent verification.
- `desktop/scripts/smoke-installed-build.mjs`: disposable signing identity and package admission.
- `desktop/scripts/verify-macos-package.mjs`: extracted archive identity, architecture, code signature and CLI version checks without GUI launch.
- `.github/workflows/ci.yml`, `.github/workflows/desktop-release.yml`: platform checks and protected production signing.
- `desktop/src-tauri/src/startup_native.rs`, `desktop/src-tauri/src/panel.rs`: installed startup and desktop lifecycle.
- `src/configuration.rs`, `src/configuration/migration.rs`: guarded legacy import and exact backup.

## Scope and Initial Inspection

The owner requested a full replacement on 2026-10-16. Read-only SSH inspection found an Apple Silicon Mac running macOS 27.0.1, with Xcode 27.1 and Rust 1.96.0. Node/npm were not available in the inspected SSH PATH or common Homebrew/version-manager locations. Build tools can stay on CI; the installed client must not require Node or Cargo.

No clipboard client process or application bundle was found in the inspected process list and standard application directories. A loaded `~/Library/LaunchAgents/ssh_clipboard.plist` still requests `/usr/local/bin/ssh_clipboard agent`, but that executable is absent and launchd reports spawn failure. The legacy `~/.config/ssh_clipboard/agent.toml` remains, with saved direct shortcuts and a startup preference. Its server name is obsolete. These are inspection results, not proof that every possible installation location has been inventoried.

The new server's current hostname initially failed SSH host verification from the Mac. The Mac's existing trusted old alias has the same Ed25519 key as the server public key retrieved through the already trusted Windows connection. Initial metadata-only Mac-to-server checks with strict verification and that existing host-key alias resolved the managed user launcher and reported server 0.4.0. Later authentication checks failed; that earlier success is not durable GUI/startup evidence. No trust file or private key was changed. Persist a verified current-hostname entry or explicitly retain the reviewed host-key alias during profile setup; repeat metadata health in the installed GUI context. Never use `StrictHostKeyChecking=no`, copy private SSH keys, or infer GUI-session authentication from a different machine's successful connection.

## Delivery Sequence

1. **Prove the application package automatically.** Build an Apple Silicon `.app.tar.gz` using installed features and a disposable updater signing key. Inspect the archived application identity/version, native executable and matching clipboard-only CLI, and verify macOS code signatures. This does not install or start the app, prove native paste, or admit a production release.
2. **Prepare protected production packaging.** Use an unused 0.4 desktop version after the package proof passes. Preserve published 0.4.1 assets and the Windows update route. A shared latest feed must retain admitted entries for every delivered platform; a Mac-only feed must not displace Windows updates. Production updater signing stays in the protected GitHub environment, with independent verification before installation.
3. **Prepare the real cutover.** Finish the actual Mac-to-server SSH trust/authentication check. Inventory exact legacy paths and startup ownership, and make a private recovery backup outside active application/configuration locations. Import existing settings without losing direct shortcut preferences; explicitly select the current server's history endpoint. No startup registration is inferred from an imported preference.
4. **Install and accept the new client.** Install the verified application in a stable user-owned location, enable its own login startup, and check the actual registered executable and `--background` argument. Use the owner's ordinary GUI session for one short native transfer/paste check, shortcut/panel check and login check. Automate package, protocol and metadata checks; guide unavoidable manual actions one at a time. Do not repeat the completed Windows test campaign.
5. **Retire the old client.** After the new client works, unload and remove the exact legacy LaunchAgent, remove verified obsolete executable links/files, and move the old active configuration/logs out of active discovery paths. Preserve one clearly identified private recovery backup, SSH keys and unrelated applications. Remove any temporary test app/startup entry created by this work. Verify that only the installed 0.4 client owns clipboard startup and shortcuts.
6. **Close the evidence.** Record exact source/package/signature identity, automated checks, native observations, cleanup inventory and any remaining update/Gatekeeper gaps. Publication remains a separate reviewed action; no planned check counts as a passed check.

Migration guardrail: initialize from the verified private copy with `ssh_clipboard settings migrate-agent --legacy-file <private-backup>/agent.toml` before first GUI launch, when shared settings are still absent. The importer records its exact recovery copy beside that source. This keeps the migration review's backup reference valid after removing the old active config directory. Existing shared settings always win; do not move their import backup without updating and validating the saved recovery reference. GUI launch or `settings show` must not be substituted for an explicit migration step.

## Signing and Update Boundaries

The existing free updater key authenticates package bytes and release identity. It does not provide Apple Developer ID or notarization. Start with the framework's supported ad-hoc macOS signature (`signingIdentity: "-"`) for the free route; it requires no Apple private signing identity. A downloaded app may still require the owner's explicit approval in macOS Privacy & Security. Do not disable Gatekeeper or remove quarantine globally. See [Tauri's signing guidance](https://v2.tauri.app/distribute/sign/macos/#ad-hoc-signing).

Native update acceptance is separate from static package signature verification. Prove an actual replacement/restart on macOS before claiming that the installed Mac updater is accepted. Do not use a disposable test key as production trust or copy the production private key to the Mac.

## Status

Inspection and the clean-cutover scope are recorded. CI now requests a release-profile, disposable-trust Mac app and retains its package/inspection evidence for seven days. The installed builder explicitly ad-hoc signs the Mac bundle, extracts the produced archive privately, and checks its bundle identity, version, native executable/CLI architecture, code signatures and CLI version. The disposable signing private key is removed before artifact upload. The inspection does not launch the GUI or change login settings.

The first package probe passed in [CI run 38919474529](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38919474529), with all seven jobs green for PR head `4a05401`. Its actual checkout/source is synthetic PR merge `3c8897a9eefd4cd09f74e8808dd1f21438a496a7`; that tree equals the reviewed head tree. The disposable 0.4.1 test archive is 8,257,323 bytes, SHA-256 `fe70cefd09ee18a97cfe396d1993530c3017d6e4419b298f07cc464acf3c0a32`. An independent Windows verifier accepted its exact package signature and authenticated identity using its explicitly disposable public trust.

On the owner's actual Mac, a private staging probe also passed archive hash, code-signature verification, CLI execution/version and migration of a copied real legacy configuration. It preserved the original target, startup preference and direct bindings, kept direct shortcuts disabled pending explicit activation, and checked the exact private import backup. Before/after hashes confirmed that real legacy/shared settings and the startup plist were unchanged. The private staging app/settings were removed and absence rechecked. This was not a GUI installation or clipboard operation. Local evidence is under the ignored `desktop/artifacts/mac-preflight-4a05401/` directory.

The SSH preflight found a separate authentication gate: the server accepts the existing public key, but the Mac's Keychain reports that user interaction is not allowed in the SSH session. The real GUI SSH agent has no loaded identities. The owner must unlock/load the existing key from the ordinary Mac session, then repeat the metadata probe using that verified GUI agent. Do not copy the private key or create an unencrypted replacement to work around this boundary. Keychain/GUI-session behavior at actual login remains a separate native check.

No production Mac installation, startup/settings change, SSH trust change or legacy retirement has taken place. macOS GUI/login/update acceptance remains open. The subsequent 0.4.2 version/notes guard changes need their own current-head CI; the earlier green run is package-foundation evidence only.

The production workflow is prepared to build Windows x64 and Apple Silicon behind the existing reviewer gate, then create one twelve-asset draft with a combined keyed feed. It pins trust to the public key in the reviewed source and refuses incomplete platform sets or disagreement on release version/trust/notes. **0.4.2** is the source candidate for this addition; it is not signed, installed or published. Its first expanded production run remains pending. Never add/replace files in the published 0.4.1 release; the published Linux server 0.4.0 remains unchanged.

Cross-platform staging guardrail: normalize CRLF notes to LF before comparing platform feeds. Windows and Mac Git checkouts can materialize different line endings for the same committed changelog; they must not cause a false notes mismatch. Other text differences still block draft creation, and checksums/signatures always cover the original package bytes unchanged.

## Update Triggers

- Mac package, startup, signing or updater behavior changes.
- Native installation or actual login/paste/update results become available.
- Legacy inventory, backup or cleanup results change.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md)
- [Compatibility and cutover](compatibility-and-cutover.md), [configuration migration](configuration-migration.md)
- [Desktop updates](desktop-updates.md), [release operations](releasing.md), [security](security.md)
