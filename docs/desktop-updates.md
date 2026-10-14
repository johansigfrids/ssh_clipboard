# Desktop Packaging and Update Trust

## Purpose

Track Phase 6 implementation and distinguish package verification from installed update evidence. The current slice implements the strict artifact policy and cryptographic fault probes. Packaging, download/update UI, startup integration, protected release workflow, and installed upgrade acceptance remain open.

## Key Files

- `desktop/src-tauri/src/update_policy.rs`: exact-byte verification and authenticated version/platform/channel bindings.
- `desktop/src-tauri/tests/fixtures/update-signing.json`: public synthetic signatures and payloads, with no private keys.
- `desktop/scripts/generate-update-test-fixtures.mjs`: disposable synthetic fixture generation and checked cleanup.
- `desktop/src-tauri/Cargo.toml`, `desktop/src-tauri/Cargo.lock`: separate desktop dependencies; updater/startup are opt-in installed-build features.

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
