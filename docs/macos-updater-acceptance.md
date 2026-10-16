# Mac Native Updater Acceptance

## Purpose

Exercise actual application replacement and restart on the owner's Mac without replacing the accepted production 0.4.2 installation. This is separate from the completed [clean migration](macos-cutover.md) and from production publication.

## Key Files

- `.github/workflows/macos-update-rehearsal.yml`: credential-free native build of two disposable packages.
- `desktop/scripts/prepare-macos-updater.mjs`: synthetic version changes, disposable signing, native package checks and private-key cleanup.
- `desktop/src-tauri/src/configuration.rs`: compile-time Mac rehearsal settings isolation.
- `desktop/src-tauri/build-support/native_update_test.rs`: isolated app identity and loopback-origin admission.
- `desktop/src-tauri/src/updates.rs`, `update_handoff.rs`: normal bounded reader, verification, installer and restart route.

## Rehearsal Boundary

The two packages use the exact Update Test product/identifier, ad-hoc Mac code signatures, a new disposable updater key, and synthetic versions 0.4.2 and 0.4.3. They are not production releases. Source evidence identifies the actual CI checkout; version-only source edits are recorded as dirty where present, then restored. The workflow has read-only repository permission, no release environment or signing secrets, and uploads only public stage files after removing the temporary private key.

Only a Mac build with the already guarded `native_update_test` configuration uses `~/Library/Application Support/ssh_clipboard-update-test/settings-v1.json`. It loads that store directly and never searches for or backs up the normal legacy config. Production/prototype Mac builds and existing Windows rehearsals keep their established settings behavior. There is no runtime configuration-path switch. Both Mac test versions use the same isolated directory so a framework restart retains the test settings.

Preserve the production app, settings and login plist with before/after hashes. Seed only the isolated test store from a validated copy, with test shortcuts and login startup disabled. Install the test app at one stable user-owned path distinct from `SSH Clipboard.app`; never overwrite an existing test app/settings directory. The test app must not register startup. Retain package, source and public-trust evidence; never transfer production signing material to the Mac.

Use the compiled loopback route on the Mac itself, with only the declared public fixture files reachable. Independently verify both archives/signatures before installing the bootstrap. Confirm the served bytes match those admitted locally. Drive the real Updates page through download/verification, separate install confirmation, actual replacement and restart. Check bundle version/code signatures, matching bundled CLI and retained isolated settings after restart. This does not prove public HTTPS delivery or browser-download Gatekeeper behavior.

After the owner quits the exact test app, stop the owned fixture and remove its exact app/settings paths only after checking the production hashes. Restore the production app if it was temporarily quit. Preserve the accepted production startup entry and the separate migration backup. No repeat production sign-out/text round trip is required.

## Status

Preparation is in progress. No test app has been installed or launched and no native Mac update has passed yet. The source adds a settings-isolation regression test and a native CI packaging workflow; successful packaging does not count as installed update acceptance. The production 0.4.2 draft remains held.

## Update Triggers

- Build, signature, package or isolation verification completes or fails.
- The owner confirms an actual update/restart or a new native issue appears.
- Test cleanup and production preservation are independently verified.

## Related Docs

- [Mac clean cutover](macos-cutover.md), [desktop updates](desktop-updates.md).
- [Release readiness](release-readiness.md), [release operations](releasing.md).
