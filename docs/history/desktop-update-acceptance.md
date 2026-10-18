# Windows Development Installer and Updater Acceptance

> Historical record, archived 2026-10-18. Version, latest-release and pending-gate statements below belong to the recorded source/session. Use [the docs index](../index.md) for current guides; preserve exact identities and unobserved limits when adding corrections.

## Purpose

Preserve the 2026-10-14 through 2026-10-16 installed Windows test results and startup investigation. This is dated evidence; the [current updater/startup guide](../desktop-updates.md) owns runtime behavior. Synthetic 0.3.x test versions are not published application versions.

## Key Files

- `desktop/scripts/prepare-native-updater.mjs`, `native-update-server.mjs`: isolated package/fixture preparation.
- `desktop/src-tauri/src/startup_native.rs`, `startup.rs`: native entry and activation boundary.
- `desktop/src-tauri/src/updates.rs`: verified install orchestration.

## Start at Login

The subsequent quoted-entry login also failed. Windows' out-of-process registry provider and startup inventory still see the old registration and no production desktop entry, while automation-process registry reads see the opposite. The installer record likewise exists only in the automation view. Treat those prior registry checks as insufficient: launch the verified installer from the owner's ordinary Windows desktop, then check `StdRegProv`/native startup inventory against direct reads. Do not write via the provider to bypass the automation context. The owner subsequently reinstalled from Windows Run, enabled startup in the Start-menu-launched app and ran scoped native cleanup. Windows' registry service verified the real installation, quoted startup and removed legacy entry; actual sign-out/sign-in launched exactly one background desktop process. This resolves that installation/startup blocker; see [the acceptance correction](release-0.4.0-acceptance.md#remaining-gates).

Production 0.4.0 investigation also found a startup quoting defect on 2026-10-16, separate from the registry-view mismatch responsible for the failed login checks. The pinned Tauri autostart wrapper passes an unquoted executable path to auto-launch 0.6, which concatenates it with arguments; the production install path contains spaces. Windows registration now calls that already-locked backend directly with a quoted absolute executable and explicit current-user enable mode. Other platforms retain the framework adapter. A Windows `CommandLineToArgvW` regression proves the executable/argument boundary for a spaced Unicode user/install path and rejects the old command representation. The signed 0.4.1 correction is now installed: the owner exercised startup off/on, and Windows' registry service confirmed removal/recreation of the correctly quoted command, enabled approval and no legacy entry. That released control saved immediately; current source uses the shared draft flow described below. Reuse the prior successful real login with that exact path/command; see [acceptance](release-0.4.1-acceptance.md). The immutable 0.4.0 desktop draft remains blocked from publication.

Guardrail: checking that a startup value exists or reports enabled does not validate its command line. Parse the exact native command and verify a real login; quote the executable independently of arguments. See [Microsoft's startup command explanation](https://devblogs.microsoft.com/oldnewthing/20210223-00/?p=104896).

Current source exposes a Behavior checkbox reflecting the **actual** startup entry until the user edits it. Saved/imported preferences remain separate and never activate registration on startup or reload. Start at login shares Apply/Discard with the other settings: only an explicit pending startup choice is sent with Apply. Unrelated saves leave external startup changes alone; unknown native state disables that checkbox while allowing other settings to save. The old immediate-save command has been removed from IPC.

Apply validates the revision and settings/shortcut syntax before preparing one shared settings write. It then changes/verifies explicitly requested startup state, activates shortcuts, and commits. Startup or shortcut failure abandons the write; commit failure restores both native activation states. Failed startup restoration reports repair guidance, and failed shortcut restoration pauses direct dispatch. The checkbox is refreshed from actual native state on failure while the pending draft stays available. See [coordinated Apply](../shared-configuration.md#startup-intent-and-coordinated-apply).

The unified draft/transaction has fake-registry, temporary-store, and rendered frontend evidence, including combined edits, startup-only preservation, unknown state, stale revisions, partial activation, commit failure, and rollback failure. The earlier native login and startup-command checks below concern released builds and their adapters; they do not prove the new combined Apply flow natively.

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

The 0.3.3/0.3.4/0.3.5 packages were staged from source `4e0d819` with version-only snapshot changes. GUI executable ProductVersion/FileVersion and the bundled CLI independently reported each installed version; the per-user `--background` startup command survived. One installed test desktop process was present after the final restart. After the final restart, the owner confirmed the saved server connection, shared a known test string, replaced the local clipboard with different text, copied the shared item back, and pasted the original test string into Notepad. This proves post-update text sharing and native clipboard retrieval for the existing connection; it does not cover the separate shelf/history/recovery acceptance pass.

This is private loopback delivery, not public GitHub/TLS or production-key acceptance. Network interruption is covered by automated real HTTP fault probes; interrupted native installation, OS publisher identity, production first-install trust and macOS/Linux installed acceptance remain open.

Cleanup also passed on 2026-10-15. After Quit, the owner uninstalled the exact Update Test application with application-data deletion unchecked. Read-only checks confirmed removal of its executable, uninstall registration and startup entry, with no test process remaining. The shared settings file's SHA-256 matched the pre-uninstall baseline; the original agent executable and startup command survived. The original agent was restored using its existing background command, with one process verified. All three owned rehearsal servers (including two failed staging attempts) stopped through their control files, and four disposable private keys were removed. Public metadata, signed test artifacts and local acceptance evidence remain for review; production signing material was untouched.

Cleanup guardrail: verify the exact test product, process command and owned fixture root before removal. Preserve shared settings and the legacy registration, use the uninstaller rather than deleting the installed directory, and compare settings hashes afterward. Source snapshots contain junctions to shared build/dependency directories; do not recursively delete those trees while following junctions.


## Update Triggers

- Correct an error in this recorded session or add a link to later evidence; keep source/version boundaries explicit.

## Related Docs

- [Current updates/startup](../desktop-updates.md), [production 0.4.0](release-0.4.0-acceptance.md), [0.4.1 correction](release-0.4.1-acceptance.md).
- [Mac updater](macos-updater-acceptance.md), [release evidence](release-readiness.md).
