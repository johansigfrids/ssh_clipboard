# Desktop Packaging and Update Trust

## Purpose

Track Phase 6 implementation and distinguish package verification from installed update evidence. Strict artifact verification, installed-build staging, and explicit startup controls are implemented. Runtime download/update UI, protected release workflow, and installed upgrade acceptance remain open.

## Key Files

- `desktop/src-tauri/src/update_policy.rs`: exact-byte verification and authenticated version/platform/channel bindings.
- `desktop/src-tauri/tests/fixtures/update-signing.json`: public synthetic signatures and payloads, with no private keys.
- `desktop/scripts/generate-update-test-fixtures.mjs`: disposable synthetic fixture generation and checked cleanup.
- `desktop/src-tauri/Cargo.toml`, `desktop/src-tauri/Cargo.lock`: separate desktop dependencies; updater/startup are opt-in installed-build features.
- `desktop/scripts/build-installed.mjs`, `smoke-installed-build.mjs`: stage/sign/verify packages; isolated synthetic-key build smoke.
- `desktop/src-tauri/tauri.installed.conf.json`, `build.rs`: installed identity, CLI sidecar, per-user NSIS, explicit release trust inputs.
- `desktop/src-tauri/src/startup.rs`, `startup_native.rs`, `panel.rs`: actual native startup state, explicit changes, and coordinated-save rollback.

## Package Staging

`cd desktop; npm run build:installed -- --version <core-version> --public-key-file <public-key>` builds the matching native CLI and desktop installer. The helper requires protected `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` inputs. Production private key files must be outside the checkout. It never installs, tags, pushes, or publishes. Release builds require explicit public-key/feed inputs; ordinary portable prototypes have no updater or startup registration.

Windows uses a per-user NSIS installer with stable desktop executable and bundled `ssh_clipboard.exe`. The CLI version must equal the chosen core version before packaging. Configuration remains in the existing separate shared-settings location. This does not add the CLI to the user's PATH. macOS/app and Linux/AppImage staging paths exist but have not been built or accepted on those platforms.

After packaging, rename the exact updater asset to its canonical version/platform filename, then sign it with `--app-version`. Verify its exact bytes with `verify_desktop_update` before writing checksums and the keyed feed. Feed filenames contain a public-key fingerprint: an old-key feed can keep serving an old-key-signed transition containing the next key and next feed. The transition helper accepts `--transition-from-public-key-file`; real installed rotation remains an acceptance gate.

`npm run test:package` generates a disposable synthetic key, builds a debug package under a separate **SSH Clipboard Update Test** application identity, signs/verifies it, and deletes the private key. It never executes the installer. On Windows x64 this passed with the generated NSIS script containing both GUI and CLI files, current-user install mode, and GUI ProductVersion/FileVersion matching core 0.3.0. The staged evidence records `testOnly: true`, `installed: false`, `published: false`. This is packaging evidence, not an installed upgrade or a production release.

Guardrail: the installed overlay exercises Tauri's generated configuration macro and sidecar path, which plain tests can miss. `serde_json` must remain a normal dependency for that macro; dev-only availability masked the initial packaging failure. Keep the real package smoke alongside source checks.

## Start at Login

Installed release builds expose a Behavior checkbox reflecting the **actual** startup entry. Saved/imported preferences are shown as desired state and never activate registration on startup or reload. Explicit changes first prepare the shared settings write, change and verify native registration, then commit settings. Save failures restore and verify the actual previous entry, even when an imported preference said otherwise; failed restoration is reported for repair. Unsaved drafts must be applied/discarded first.

The entry names the stable installed app and passes `--background`; startup/second-instance background launches do not reveal the panel or read the clipboard. The legacy agent's entry is untouched. Prototype/debug builds cannot modify startup. Tests use a fake registry, including registration/save/rollback faults; no actual OS startup entry or login has been exercised. Windows startup suppression outside the registration API still requires native login acceptance.

## Package Policy

The chosen updater remains Tauri's supported installer/update path. The current pinned plugin exposes required signed-version verification, and the installed CLI's signer supports `--app-version`. Keep those enabled rather than accepting older signatures without a signed version.

The application policy additionally uses Minisign's verified trusted comment to bind the artifact's exact filename. The filename encodes stable channel, platform/architecture, and canonical version. `SSHClipboard-stable-windows-x86_64-2.0.0-setup.exe` is a synthetic fixture name, not a chosen production release.

Verify the global signature before reading trusted-comment fields. Require exactly one matching `version:` and `file:` field. A valid signature for another version, platform, or channel cannot be relabelled by an unsigned feed. Accept only strictly newer stable SemVer releases without build metadata. Candidate URLs must match the canonical repository's HTTPS `desktop-v<version>` asset path and expected filename. The byte policy caps artifacts at 256 MiB; the future downloader must enforce this budget while reading, before allocating the whole response.

The entire feed and its release notes are not claimed to be signed. Version and platform/channel selection are authenticated through the trusted comment; payload bytes are covered by the package signature. Key rotation needs an old-key-signed transition build containing the new public key and its feed location. Keep an old-key feed serving that transition for clients that skip releases. Replacing an unsigned feed entry does not install a new trust key.

## Keys and Evidence

This implementation has not created or configured a production signing key. Fixture generation creates two synthetic private keys in a disposable ignored directory, writes only public keys/signatures and synthetic payloads into the test fixture, then validates the absolute cleanup path and removes the temporary directory. Private fixture material is not committed. Regeneration uses the existing local Tauri CLI and prints only a completion message.

Automated checks accept a valid synthetic package and reject changed bytes, wrong keys, missing signatures, replay/downgrade, forged version metadata, and valid signatures for another channel/platform. A cryptographic transition probe verifies an old-key-signed payload containing the new public key, then verifies the next payload with the new key and rejects it with the old key. This does not prove an installed application has upgraded or rotated trust.

The updater key is separate from SSH identities and OS publisher signing. Real keys require a backup outside source/build output and protected release-only credentials. These tests do not install an app, register startup, or publish a release.

## Update Triggers

- Runtime updater, package naming, download limits, trust binding, or key/feed transition changes.
- Installer/startup support, protected release workflow, or installed upgrade evidence changes.

## Related Docs

- [Releasing](releasing.md), [CI](ci.md), and [testing](testing.md)
- [Architecture](../ARCHITECTURE.md) and [implementation plan](implementation-plan.md)
- Official [Tauri updater](https://v2.tauri.app/plugin/updater/) and [Windows installer](https://v2.tauri.app/distribute/windows-installer/) documentation
