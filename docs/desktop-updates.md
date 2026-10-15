# Desktop Packaging and Update Trust

## Purpose

Track Phase 6 implementation and distinguish package verification from installed update evidence. Strict verification, installed staging, explicit startup, bounded update discovery/download, guarded installer handoff, and a protected draft-release workflow are implemented. Isolated native Windows installation, sign-in, built-in upgrades, download cancellation, tamper rejection and an installed signing-key transition pass. Protected production signing and public delivery remain open.

## Key Files

- `desktop/src-tauri/src/update_policy.rs`: exact-byte verification and authenticated version/platform/channel bindings.
- `desktop/src-tauri/tests/fixtures/update-signing.json`: public synthetic signatures and payloads, with no private keys.
- `desktop/scripts/generate-update-test-fixtures.mjs`: disposable synthetic fixture generation and checked cleanup.
- `desktop/src-tauri/Cargo.toml`, `desktop/src-tauri/Cargo.lock`: separate desktop dependencies; updater/startup are opt-in installed-build features.
- `desktop/scripts/build-installed.mjs`, `smoke-installed-build.mjs`: stage/sign/verify packages; isolated synthetic-key build smoke.
- `desktop/src-tauri/tauri.installed.conf.json`, `build.rs`: installed identity, CLI sidecar, per-user NSIS, explicit release trust inputs.
- `desktop/src-tauri/src/startup.rs`, `startup_native.rs`, `panel.rs`: actual native startup state, explicit changes, and coordinated-save rollback.
- `update_feed.rs`, `update_network.rs`, `updates.rs`: untrusted feed parsing, bounded HTTPS, cancellation, and independent update state.
- `update_handoff.rs`, `tests/framework_handoff.rs`: one-use local metadata bridge and actual framework check with a mock runtime.
- `.github/workflows/desktop-release.yml`: manual master-only validation and environment-gated signed draft creation.
- `desktop/scripts/prepare-native-updater.mjs`, `native-update-server.mjs`: opt-in local native rehearsal packages and controlled feed faults.
- `desktop/src-tauri/build-support/native_update_test.rs`: build-time identity/origin gate and canonical test routing.

## Runtime Update Path

Installed release builds check after a short startup delay and every six hours without opening the panel, sampling the clipboard, or downloading/installing a package. Checking uses independent state, so a quiet network request does not hold the transfer/session lock. The Updates page offers an explicit check, download/verify, then separate install confirmation. Notes are bounded inert text. A prepared verified package is retained in RAM until install, a new explicit check, or Quit; quiet polling does not invalidate it.

Native rehearsal exposed a flash when rechecking a visible offer: checking immediately removed the offered version/notes/buttons, while a global operation banner pushed the settings content down, then both changes reversed on the fast response. Rechecks now retain the displayed card while invalidating cached install bytes/offer and disabling actions. The result replaces the card once; failure/cancellation clears the stale display, while a failed download retains valid retry metadata. Progress/Cancel stay in a reserved inline area within Updates, with an inline Checking button state rather than inserting a banner above Settings. Rendered regression tests retain actual header/card nodes across a pending check; Rust tests cover failed-check versus download-retry metadata. On installed test 0.3.3, the owner confirmed that rechecking the 0.3.4 offer kept the card/notes in place without a flash or page jump.

Remote requests enforce HTTPS and platform TLS verification. Only GitHub and its known release asset hosts are allowed, including redirects, with at most five redirects. Metadata reads cap at 64 KiB/20 seconds, notes at 16 KiB, and package reads at 256 MiB/300 seconds. Check declared lengths before allocation and streamed lengths before append; missing or false lengths do not bypass the budget. Cancel drops the response and partial bytes. There is no automatic resume, download, install, or clipboard replay. Interrupted and oversized synthetic HTTP responses are tested without relaxing production HTTPS.

The current framework's `check()` and `download()` buffer remote bodies without application caps. Do not call them on remote URLs. Our reader validates a candidate, downloads bounded bytes, verifies the package, then uses a one-use **127.0.0.1** metadata bridge solely to construct the framework's supported installer object. It has a random route, 4 KiB request cap, fixed bounded response, five-second lifetime, and abort-on-drop ownership. The object's version/URL/signature must still equal the verified candidate. It never calls the framework downloader. Reverify the exact owned bytes immediately before handoff; no frontend-supplied paths or package bytes are accepted.

The installed overlay enables the framework's plain-HTTP setting **only for that loopback bridge** and leaves its default endpoints empty. All remote reads independently require HTTPS. No frontend updater plugin permissions are granted. Replacing this bridge with the library's remote check would remove the metadata bound and widen that setting; keep the adapter tests and these constraints when updating dependencies.

Installation reserves the operation/session boundary, requires idle native clipboard work and resolved/discarded uncertain shares, and disables new transfers until completion/failure. Once installer execution starts, cancellation is unavailable. The framework Windows pre-exit hook runs before process launch and can be followed by launch failure: stop/reap SSH owners there, retain native app/clipboard owners, and restore background transport tasks if launching fails. Normal Quit still joins owners and stops the clipboard worker. A successful Windows handoff exits; macOS/AppImage use framework replacement and relaunch, with native acceptance still required. There is no automatic installer rollback promise.

## Verification and Release Gates

Windows checks pass for strict crypto, bounded feed/stream reads, interruption, loopback lifetime/route rejection, and actual framework candidate selection using Tauri's mock runtime. Rendered tests cover inert notes, no automatic download/install, explicit confirmation, and transfer/uncertain-share guards. A fresh isolated NSIS debug package with this source was built, signed, and verified; it was not installed or published.

The framework mock probe imports `TaskDialogIndirect` on Windows. Put it in an integration-test target and supply the common-controls manifest with **test-scoped** linker flags. Missing manifests produce `STATUS_ENTRYPOINT_NOT_FOUND`; global flags conflict with the app's existing manifest resource. The full test suite and real package smoke must both pass after build-script changes.

Release review found that the non-Windows post-install path called synchronous cleanup directly from the async install command. Cleanup joins transport owners through Tauri/Tokio `block_on`, which cannot nest inside that async runtime. Post-install cleanup now runs on a blocking worker before restart; a real runtime-boundary probe proves that its synchronous callback can join an async owner without a nested-runtime panic. Normal GUI-exit cleanup and the Windows installer's blocking pre-exit hook retain their existing context. This source/runtime test does not claim an installed macOS/Linux upgrade.

Cross-platform CI also checks the installed-feature command itself: Tauri's non-Windows `restart()` never returns, so only the Windows branch has a success return afterward. The enclosing async result is explicitly `Result<(), String>` so both platform branches retain the same error type when that success expression is absent. Keep platform-specific return paths inside their matching configuration gates; Windows-only checks cannot detect unreachable non-Windows code or inference changes.

The new workflow is manual, master-only, checks the immutable dispatch SHA/version, validates without credentials, and stages a Windows x64 draft only after the `desktop-release` environment gate. Actions use resolved commit pins, Rust is pinned, Cargo/npm locks are used, release jobs do not restore PR build caches, and signing inputs are scoped to the package step. It never publishes or overwrites an existing release. This workflow has been source-checked locally but has not run on GitHub; the owner has since configured environment reviewers/ref rules and production signing inputs, with metadata/public-key consistency verified on 2026-10-14. Actual protected signing remains unverified. Other platforms stay outside this initial release job until native package/upgrade gates pass. See [release/key operations](releasing.md).

## Package Staging

`cd desktop; npm run build:installed -- --version <core-version> --public-key-file <public-key>` builds the matching native CLI and desktop installer. The helper requires protected `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` inputs. Production private key files must be outside the checkout. It never installs, tags, pushes, or publishes. Release builds require explicit public-key/feed inputs; ordinary portable prototypes have no updater or startup registration.

Windows uses a per-user NSIS installer with stable desktop executable and bundled `ssh_clipboard.exe`. The CLI version must equal the chosen core version before packaging. Configuration remains in the existing separate shared-settings location. This does not add the CLI to the user's PATH. macOS/app and Linux/AppImage staging paths exist but have not been built or accepted on those platforms.

After packaging, rename the exact updater asset to its canonical version/platform filename, then sign it with `--app-version`. Verify its exact bytes with `verify_desktop_update` before writing checksums and the keyed feed. Feed filenames contain a public-key fingerprint: an old-key feed can keep serving an old-key-signed transition containing the next key and next feed. The transition helper accepts `--transition-from-public-key-file`; the isolated installed Windows rotation passed as recorded below.

`npm run test:package` generates a disposable synthetic key, builds a debug package under a separate **SSH Clipboard Update Test** application identity, signs/verifies it, and deletes the private key. It never executes the installer. On Windows x64 this passed with the generated NSIS script containing both GUI and CLI files, current-user install mode, and GUI ProductVersion/FileVersion matching core 0.3.0. The staged evidence records `testOnly: true`, `installed: false`, `published: false`. This is packaging evidence, not an installed upgrade or a production release.

Guardrail: the installed overlay exercises Tauri's generated configuration macro and sidecar path, which plain tests can miss. `serde_json` must remain a normal dependency for that macro; dev-only availability masked the initial packaging failure. Keep the real package smoke alongside source checks.

## Start at Login

Installed release builds expose a Behavior checkbox reflecting the **actual** startup entry. Saved/imported preferences are shown as desired state and never activate registration on startup or reload. Explicit changes first prepare the shared settings write, change and verify native registration, then commit settings. Save failures restore and verify the actual previous entry, even when an imported preference said otherwise; failed restoration is reported for repair. Unsaved drafts must be applied/discarded first.

The entry names the stable installed app and passes `--background`; startup/second-instance background launches do not reveal the panel or read the clipboard. The legacy agent's entry is untouched. Prototype/debug builds cannot modify startup. Automated tests use a fake registry, including registration/save/rollback faults. Native test-app registration, direct background launch and actual Windows sign-out/sign-in have now been exercised as recorded below.

Native Windows acceptance on 2026-10-14 installed an isolated release-mode `SSH Clipboard Update Test` 0.3.0 with disposable trust. The owner confirmed launch and the saved connection; the actual per-user executable/installer registration reported 0.3.0. Enabling startup registered the installed test executable with `--background`. A launch with those exact arguments reached the tray, but the owner saw the panel flash before hiding. The configured window was visible during native creation, before setup could hide it. It now starts hidden and unfocused; setup explicitly shows normal launches, or a background launch whose tray failed. The embedded-context probe guards the initial configuration.

On 2026-10-15, an isolated source snapshot of fix `7693935` was staged as test version 0.3.1 without changing the repository's release version. Its signed package passed admission against the original test public key/current 0.3.0. The owner manually upgraded without uninstalling, launched the installed app and confirmed 0.3.1. Read-only checks confirmed GUI/CLI/installer registration all reported 0.3.1 and the exact startup command survived. After Quit and a fresh background launch, the owner confirmed no panel flash; clicking the tray opened the panel with the saved connection. The owner then signed out of Windows and back in: the test app started automatically without a panel flash. Two tray icons were observed; read-only process/startup inspection identified one preserved legacy `ssh_clipboard_agent.exe --autostart` and one installed test `ssh_clipboard_desktop.exe --background`, each with its own existing registration. This is migration coexistence, not a duplicate desktop instance. Keep the old entry untouched until an explicit cutover or test cleanup, and distinguish icons by the legacy Push/Pull/Peek menu versus the desktop Open clipboard/Settings menu. These are native installer/manual-upgrade and actual sign-in results. No production installation, publication or production signing-key use occurred.

## Installed Windows Update Results

On 2026-10-15, the owner manually installed the isolated loopback-capable 0.3.2 bootstrap, then exercised the real Updates UI and installer using disposable signing keys:

- Same-key 0.3.2 to 0.3.3: download verification, separate install confirmation, replacement and restart succeeded.
- A deliberately slow 0.3.4 download stopped on Cancel, displayed `Cancelled` and left 0.3.3 installed.
- Altered 0.3.4 bytes failed signature/release-identity verification, exposed no Install button and left 0.3.3 installed. Restoring the valid bytes allowed a fresh verified download.
- Old-key-signed 0.3.4 installed and restarted successfully with the next public key/feed embedded. The old-key feed retained the 0.3.4 transition while the next-key feed offered 0.3.5.
- Installed 0.3.4 discovered and verified next-key-signed 0.3.5, then installed and restarted at 0.3.5 after a separate confirmation. This proves the installed trust transition through the real Windows updater path.

The 0.3.3/0.3.4/0.3.5 packages were staged from source `4e0d819` with version-only snapshot changes. GUI executable ProductVersion/FileVersion and the bundled CLI independently reported each installed version; the per-user `--background` startup command survived. One installed test desktop process was present after the final restart. Saved connection retention was confirmed earlier in the installation sequence; final native transfer/configuration checks are separate.

This is private loopback delivery, not public GitHub/TLS or production-key acceptance. Network interruption is covered by automated real HTTP fault probes; interrupted native installation, OS publisher identity, production first-install trust and macOS/Linux installed acceptance remain open. Test application/server/key cleanup is still pending.

## Isolated Native Updater Rehearsal

`node desktop/scripts/prepare-native-updater.mjs --state-file <owned-test-acceptance-state>` stages four release-mode Windows test packages from isolated version-only source snapshots: a loopback-capable 0.3.2 bootstrap, same-key 0.3.3, old-key-signed 0.3.4 embedding the next test key, and next-key-signed 0.3.5. It requires the existing disposable test workspace/key, captures new signer output, and never installs, publishes, reads production signing material or changes the repository's release version. It leaves an owned loopback fixture and disposable keys for the supervised test, to be stopped/removed during cleanup.

Add `--refresh` to stage a revised 0.3.3/0.3.4/0.3.5 sequence for that same disposable fixture. It retains the installed bootstrap/origin/test trust, writes new immutable revision directories, and switches only the local test routing atomically after every revised package verifies. Failed refreshes leave the existing fixture routing intact. Recheck in the installed test app to load the revised candidate signature before downloading. This is a mutable private rehearsal feed, never permission to overwrite a published tag or signed release asset.

The fixture transport is compiled only when the explicit `--test-build --native-test-origin` staging options supply a literal `127.0.0.1` origin with an unprivileged port and random 128-bit route. Inherited fixture environment inputs are refused. The Rust build script independently requires the exact Update Test product/identifier and installed feature; production/prototype identities fail the build. Ordinary builds have no fixture dispatch compiled, including ordinary all-feature CI checks. Native fixture builds keep canonical GitHub candidate/feed selectors, signed version/filename checks, byte limits, progress/cancellation and the real framework installer handoff. Only transport resolution changes; it disables proxy/redirects and serves declared files under the owned fixture root. It adds no runtime override or production trust exception. This loopback rehearsal does not prove public GitHub/TLS delivery.

The local `control.json` selects an exact offered test version and mode (`valid`, `slow`, `tampered`, `interrupted`, or `stop`). Feed routes are keyed by signing identity; choosing an older signed feed exercises the existing no-downgrade offer rule. Package modes let the owner cancel a real download or observe signature/interruption rejection before successful installation. Metadata-only request logs record route/mode/size, never clipboard content or private keys. Automated probes cover identity/origin refusal, owned-file routing, changed bytes, interruption and cancellation. A real production-identity build with a fixture input was rejected as expected. Built-in Windows installation, cancellation, tamper rejection and key transition now have the separate native evidence above.

Windows native staging verifies Cargo with `where.exe` and prepends that existing directory only inside the rehearsal process. Native CLI discovery failed after sign-in when an unmatched quote in the inherited PATH hid later Rust entries, although direct Cargo invocation worked. The scoped prefix restored tool discovery without editing Windows environment settings or installing tools.

## Package Policy

The chosen updater remains Tauri's supported installer/update path. The current pinned plugin exposes required signed-version verification, and the installed CLI's signer supports `--app-version`. Keep those enabled rather than accepting older signatures without a signed version.

The application policy additionally uses Minisign's verified trusted comment to bind the artifact's exact filename. The filename encodes stable channel, platform/architecture, and canonical version. `SSHClipboard-stable-windows-x86_64-2.0.0-setup.exe` is a synthetic fixture name, not a chosen production release.

Verify the global signature before reading trusted-comment fields. Require exactly one matching `version:` and `file:` field. A valid signature for another version, platform, or channel cannot be relabelled by an unsigned feed. Accept only strictly newer stable SemVer releases without build metadata. Candidate URLs must match the canonical repository's HTTPS `desktop-v<version>` asset path and expected filename. The byte policy caps artifacts at 256 MiB; the future downloader must enforce this budget while reading, before allocating the whole response.

The entire feed and its release notes are not claimed to be signed. Version and platform/channel selection are authenticated through the trusted comment; payload bytes are covered by the package signature. Key rotation needs an old-key-signed transition build containing the new public key and its feed location. Keep an old-key feed serving that transition for clients that skip releases. Replacing an unsigned feed entry does not install a new trust key.

## Keys and Evidence

The keyed-feed URL uses GitHub's repository-wide latest release. Server drafts opt out with `--latest=false`, server publication must retain that flag, and the legacy release action uses `make_latest: false`. Only a reviewed desktop release carrying the keyed feeds may become latest. See [publication and recovery rules](releasing.md#key-rotation-and-recovery); signed admission does not make a missing feed discoverable.

The owner generated the production signing identity locally and confirmed its encrypted private key/password are retained in the password manager. Read-only checks on 2026-10-14 verified protected environment metadata, expected secret names and matching public-key variables; no private value was read and no protected production build has signed yet. Fixture generation creates two synthetic private keys in a disposable ignored directory, writes only public keys/signatures and synthetic payloads into the test fixture, then validates the absolute cleanup path and removes the temporary directory. Private fixture material is not committed. Regeneration uses the existing local Tauri CLI and prints only a completion message.

Automated checks accept a valid synthetic package and reject changed bytes, wrong keys, missing signatures, replay/downgrade, forged version metadata, and valid signatures for another channel/platform. A cryptographic transition probe verifies an old-key-signed payload containing the new public key, then verifies the next payload with the new key and rejects it with the old key. This does not prove an installed application has upgraded or rotated trust.

The updater key is separate from SSH identities and OS publisher signing. Real keys require a backup outside source/build output and protected release-only credentials. These tests do not install an app, register startup, or publish a release.

## Update Triggers

- Runtime updater, package naming, download limits, trust binding, or key/feed transition changes.
- Installer/startup support, protected release workflow, or installed upgrade evidence changes.

## Related Docs

- [Releasing](releasing.md), [CI](ci.md), and [testing](testing.md)
- [Architecture](../ARCHITECTURE.md) and [implementation plan](implementation-plan.md)
- Official [Tauri updater](https://v2.tauri.app/plugin/updater/) and [Windows installer](https://v2.tauri.app/distribute/windows-installer/) documentation
