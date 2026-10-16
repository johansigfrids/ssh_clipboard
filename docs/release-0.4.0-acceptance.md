# 0.4.0 Production Artifact Acceptance

## Purpose

Record acceptance of the immutable signed 0.4.0 drafts, separately from source tests, earlier disposable-trust rehearsals, publication and live migration.

## Key Files

- `.github/workflows/desktop-release.yml`, `server-release.yml`: protected draft builders.
- `desktop/src-tauri/src/bin/verify_desktop_update.rs`, `src/server_package.rs`: independent package admission.
- [Release readiness](release-readiness.md) and [cutover procedure](compatibility-and-cutover.md): remaining gates.

## Source and Trust

On 2026-10-16, PR [#1](https://github.com/johansigfrids/ssh_clipboard/pull/1) merged at `766f6e2659ecf984fdb568da10b8d941d07486d9`. Its tree matches the reviewed feature head. The [merged-master CI run](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38889968185) passed all seven jobs.

The owner approved both protected environments. [Desktop run 38890224506](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38890224506) and [server run 38890227589](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38890227589) passed validation and production signing from that exact commit. Both releases remain drafts: `desktop-v0.4.0` and `server-v0.4.0`. The latest public release remains `v0.3.0`.

Independent verification used retained public trust with SHA-256 `08c20604ca8fb74860ff1441426e3aa1d85f2ebf6acdbf44ef890d077bdcaab7` (key ID `08c20604ca8fb748`). Production private keys/passwords were consumed only inside the protected jobs; the assistant did not read them.

| Exact package | Bytes | SHA-256 |
| --- | ---: | --- |
| `SSHClipboard-stable-windows-x86_64-0.4.0-setup.exe` | 4858206 | `2f99ff225534417cd4aa5a4f2883bdc43be28bd25ad2beae1a42607df6e99bd8` |
| `SSHClipboardServer-stable-linux-x86_64-0.4.0` | 6854480 | `05b5e3790b777bb8e3b9a02641671802b2a8c1cbe2678f86db9e146c5dad43ec` |

Downloaded canonical assets, source/profile/version/platform/key evidence, sizes, checksums and authenticated package signatures passed independent admission. Desktop candidate and keyed-feed selectors agree with the canonical installer and detached signature; its proof records clean source, release profile, matching CLI and no test transport. These are draft downloads using authenticated GitHub access, not public stable-feed delivery.

## Windows Production Installation

After separate owner authorization, the exact verified installer was launched and the original 0.3.0 agent temporarily stopped. Its executable and startup entry were preserved. Three exact configuration backups and their hashes were retained locally outside tracked source.

The owner completed installation. Read-only checks confirmed one production desktop process, per-user installer registration at 0.4.0, GUI product/file versions 0.4.0 and bundled CLI reporting `ssh_clipboard 0.4.0`. The shared settings, prior schema backup and legacy agent TOML matched their pre-install hashes; the legacy startup command remained unchanged.

The owner confirmed the saved server connection and this native round trip through the existing legacy endpoint: Share `SSH Clipboard production 0.4.0`, replace the local clipboard with another marker, Copy the shared item, then paste the original shared text into Notepad. This admits production installation and that text path. It does not claim production startup/sign-in, a production-to-production update, actual legacy-file import, shelf cutover or interrupted installation.

The [closed native history session](native-history-acceptance.md) and [installed updater rehearsals](desktop-updates.md) retain their original evidence boundaries; they were not repeated.

After that successful production check, the owner explicitly requested complete removal of the old agent. Its own uninstaller removed the legacy startup/PATH entries, agent/CLI binaries and bin directory with seven successful checks and no warnings. A temporary copy of the CLI avoided deferred self-deletion and was then removed. The validated legacy configuration subtree contained only the agent TOML and empty dated logs; it and the empty old installation root were removed after verifying the retained recovery backup and rejecting reparse points. The new shared settings hash was unchanged and one production desktop process remained. The Linux service was untouched. The real-profile startup cleanup and login result are recorded below.

## Linux Production Artifact

The verified exact optimized server reported 0.4.0 under WSL. Pinned actual published v0.3.0 CLI/proxy/daemon compatibility passed all eight private text/PNG/history/response-budget checks against it, with cleanup.

On the authorized Ubuntu test host, real SSH protected Share, deduplicated retry, exact text bytes, Watch, reconciliation and deletion passed against a temporary daemon. A separate guarded real user-systemd fixture installed the same signed artifact under a temporary HOME, launcher, private socket and runtime-only unit. Enable/start and selected-version protocol health passed; a forged 0.4.1 request using 0.4.0 bytes was refused without changing selection or service. The retained executable survived deletion of the uploaded input, and restart preserved custom options.

Owned processes, runtime unit and temporary data were removed. The installed legacy service's PID/invocation/state/path and existing SSH route matched their original baselines. No live service or SSH restriction was replaced. Earlier disposable-debug crash-loop rollback/journal-recovery evidence remains separate; no extra production release was signed for that fault.

## Remaining Gates

**Authoritative registry discrepancy found after the second failed login:** the quoted repair also failed the owner's real sign-in check. Direct HKCU/HKU reads from the automation process show the new entry and no legacy entry, but Windows' out-of-process `StdRegProv` sees the legacy `ssh_clipboard` command and no `SSH Clipboard` entry. `Win32_StartupCommand` agrees with that service view. The same provider finds no production desktop uninstall record, despite the automation-view record. The native files exist and direct launches work; no matching application/Code Integrity/AppLocker/Defender error was found. Therefore automation-view registry checks do not establish real-profile installation/startup or complete legacy-registration cleanup. This was resolved by reinstalling from the owner's ordinary Windows desktop and verifying the service view before another login test. Do not use the provider to write around the automation boundary. The quoting change remains a valid command-boundary correction, but it did not establish the cause of these failed launches.

**Resolved Windows acceptance:** the owner reopened the verified installer using Windows Run, completed installation, opened the app from the Start menu and enabled startup. Windows' registry service then confirmed the production 0.4.0 installer record and new startup command. The owner ran a scoped local cleanup script from normal PowerShell; it refused mismatched registry views before writing, backed up the actual values, quoted the new command, removed the old Run/StartupApproved/PATH entries and verified unchanged shared settings. Read-only service queries confirmed the result independently.

After signing out and back in, the owner confirmed that the app started. Inspection found exactly one production desktop process in the new session, launched from the installed path with `--background`; the real registry retained the quoted command and no legacy Run entry. This closes real-profile installation, legacy startup removal and actual login launch. No explicit production panel-flash observation was supplied, so do not extend that claim beyond the earlier isolated native evidence.

**Desktop publication held for source correction:** prepare the quoting adapter as 0.4.1 with a new signed package after CI/review. Retain the immutable 0.4.0 desktop draft bytes as evidence and do not publish or overwrite them. The independent signed server-v0.4.0 draft remains under its existing acceptance/release gates. Reinstalling the published legacy release with the retained configuration backup is the client return path.
- Review persistent Linux unit-directory ownership/permissions (the earlier probe found group-writable mode `0775`), installation/login/boot and the concrete legacy-service/socket/SSH transition before separately authorizing live migration. Restart loses in-memory clipboard items.
- Public production delivery, interrupted native installation and omitted native recovery paths remain explicit gaps. macOS/Linux desktop installers are not accepted platforms.
- Publication is separate. Do not overwrite signed draft bytes or create a legacy `v0.4.0` tag. Server publication must use `--latest=false`; only the reviewed desktop keyed-feed release may become latest.

Guardrail: source, protected signing, independent byte verification, native paste and installed-service evidence answer different questions. A successful draft or installer process launch alone does not close the next gate. Documentation-only follow-up commits do not change the immutable source of these artifacts.

## Update Triggers

- Production startup, persistent service, public delivery or another exact-artifact gate gains evidence.
- A draft is published or a separately reviewed live transition is authorized/completed.

## Related Docs

- [Release operations](releasing.md), [desktop updates](desktop-updates.md), [server maintenance](server-maintenance.md).
- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md), [docs index](index.md).
