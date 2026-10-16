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

Native replacement/restart and cleanup passed on 2026-10-16. [PR #8](https://github.com/johansigfrids/ssh_clipboard/pull/8) merged as `1e3dd5d48aae42b3aa704ee0244c6ec385d8bf21` after all seven regular [CI checks](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38941083727) and the dedicated [two-package workflow](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38941083608) passed for head `109e65dda1fdc3d9cb819600ac27152cc81b4e8c`. The actual package checkout is synthetic PR merge `4c5ca2cfae1674423b85ada5b338ed8a6f799631`; its tree, the reviewed head and the real merge all equal `8e91b6d7f1ca24cf5faa8da000fe1f0b8bcf0947`.

The offline verifier independently accepted both archives with explicit disposable public trust `108705b0e4d1b10e`, distinct from production trust. Native Mac inspection also passed bundle identity, architecture, code signatures and CLI version. The exact packages are:

| Test version | Bytes | SHA-256 |
| --- | ---: | --- |
| 0.4.2 bootstrap | 8,162,807 | `14c6dafdc40cf525647202fc110060cfba89454972c988b722580619d9d9ef33` |
| 0.4.3 update | 8,162,998 | `8d9c74e5db9d700890d7a8dc7fd3cf8138259e0c4f3e780cafc3ab13932f36b3` |

The Mac-local fixture served the exact admitted 0.4.3 bytes and rejected requests outside its capability route. The owner opened the separate 0.4.2 test app, saw the isolated Mac update rehearsal profile, discovered 0.4.3, confirmed successful download verification, received a separate install confirmation, then confirmed actual replacement and restart at 0.4.3. Automated post-update inspection found exactly one updated test process; strict code signatures, both executable hashes and bundled CLI version matched the admitted 0.4.3 archive. The isolated settings hash remained identical across replacement.

Production app/CLI/Info.plist, settings and startup-plist hashes matched their pre-test baselines. The production updater implementation files (`updates.rs`, `update_handoff.rs`, `update_network.rs`, `update_policy.rs`) are unchanged from signed production source `b465c4407805f4feb8d4f58cb9d28e209e26ed7e`. This is installed framework-path evidence with isolated transport/configuration and synthetic version/trust; it does not turn either synthetic package into a production release.

After the owner quit the exact test app, guarded cleanup verified no remaining test process, removed its app and isolated settings, stopped the verified owned fixture process and checked the port was released. It then removed the exact private test directory, including the temporary settings copy. Metadata-only request logs and receipts remain in ignored `desktop/artifacts/TEST-only-macos-updater-38941083608/`; local operator state is under ignored `target/macos-updater/`. No test startup entry was created. The original production 0.4.2 process remained running; its independent final audit reports one desktop process, the accepted background startup path, retained recovery reference and successful server health. The separate original migration backup is preserved.

No production signing credentials were used or transferred. Public HTTPS/native first-download behavior, browser-download Gatekeeper approval, native Mac image paste, interrupted replacement and Mac native signing-key rotation are not claimed. The existing Windows fault/rotation campaign remains separate evidence. The production 0.4.2 draft is still held for separate publication approval; this documentation follow-up does not change its signed source or package bytes.

## Update Triggers

- Build, signature, package or isolation verification completes or fails.
- The owner confirms an actual update/restart or a new native issue appears.
- Test cleanup and production preservation are independently verified.

## Related Docs

- [Mac clean cutover](macos-cutover.md), [desktop updates](desktop-updates.md).
- [Release readiness](release-readiness.md), [release operations](releasing.md).
