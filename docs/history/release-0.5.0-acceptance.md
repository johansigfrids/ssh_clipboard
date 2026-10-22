# 0.5.0 Publication and Migration Acceptance

> Historical record, 2026-10-22. Results apply to the exact source and packages below. Use [the docs index](../index.md) for current guides.

## Purpose

Record the authorized desktop/server 0.5.0 publication, independent artifact checks and completed live migration on 2026-10-22, including owner-confirmed Windows/Mac clipboard checks and cleanup. This release requires [fresh settings and server setup](../compatibility-and-cutover.md); publication and live cutover are separate operations.

## Key Files

- `.github/workflows/desktop-release.yml`, `.github/workflows/server-release.yml`: protected signed staging.
- `desktop/release-notes/0.5.0.md`: committed desktop release notes.
- `docs/update-signing-public-key.pub`: independently pinned production trust.
- [Release operations](../releasing.md), [server setup](../server-setup.md), [changelog](../../CHANGELOG.md).

## Source and Automated Gates

[PR #31](https://github.com/johansigfrids/ssh_clipboard/pull/31) merged tested head `790352d21aba21f1f8796da6922c39593b52dc24` as `8c8cc709f963c811b16d29be680926cf46d4b3df`. Both published tags point to this commit; later documentation commits do not change the signed source.

[PR CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39775813232) and [post-merge CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39778353588) passed core checks on Windows/Mac/Linux, desktop checks on all three platforms, browser contracts, synthetic Windows/Mac packages, signed server staging and disposable install/update/rollback/recovery. A Linux dependency-download delay was retried on a fresh runner before merging. The [Mac updater rehearsal](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39775813275) also passed after its obsolete 0.4.2 version anchor was replaced with the committed baseline and next patch.

[Desktop staging](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39778358660) and [server staging](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39778362365) passed credential-free validation and protected native signing. Both environments retained master-only rules, owner-review gates and public trust matching the reviewed source. Public key ID remains `08c20604ca8fb748`.

## Exact Packages and Independent Verification

| Platform | Package bytes | SHA-256 |
| --- | ---: | --- |
| Windows x64 | 4,521,251 | `a099cebf2d78e434e9b70ee01f5480db51a86623b55a74d8fb84848fd3a421d3` |
| Apple Silicon Mac | 7,477,632 | `586ddf369af4d913e240708d3737e072107d340c69384cde0837bf9691c44fcc` |
| Linux x64 server | 5,740,240 | `e2b8e0f0cfd96aefaf71178d90933dbdddfaa16bb64d81eb953df96e0823fadc` |

All twelve desktop and five server assets were downloaded and checked against native workflow artifacts, exact source/version/platform, release profile, size/hash/checksum evidence, candidates and signatures. Offline verifiers accepted all three packages against the repository-pinned public key. Desktop admission also checked clean production source, matching bundled CLI versions, committed notes and the combined keyed feed. The Mac receipt records arm64 bundle/CLI identity, archive inspection and native code-signature verification.

The exact independently verified server binary ran in a disposable WSL directory with a private daemon/socket and synthetic SSH shim. Version, removed-command refusal, Health, Share/List, exact-byte Fetch, epoch-guarded Delete and direct-target settings isolation passed. The daemon was terminated and its temporary files removed. This exercised no actual SSH route or systemd service.

## Publication and Anonymous Delivery

The owner requested committing, a PR and the new releases. [Server 0.5.0](https://github.com/johansigfrids/ssh_clipboard/releases/tag/server-v0.5.0) was published at 12:38:11 UTC with explicit `--latest=false`; desktop 0.4.3 remained latest at that point. [Desktop 0.5.0](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.5.0) was published at 12:46:45 UTC and became latest after independent combined-draft verification.

Anonymous HTTPS downloads matched all seventeen verified draft assets byte-for-byte. All three public packages passed signature/authenticated-identity verification again. The repository-wide latest keyed-feed URL serves the reviewed combined 0.5.0 Windows x64/Apple Silicon feed. Both releases are stable and non-draft; neither publication rebuilt or replaced signed assets.

Ignored local workflow artifacts, draft/public downloads and verification receipts are under `target/release-0.5.0/`.

## Completed Live Server Migration

After publication, the owner authorized replacement of the existing installations, explicitly declined rollback retention and approved permanent removal of old settings and startup entries. The live migration used the exact signed packages listed above; it did not rebuild or republish them.

The Linux x64 server previously ran 0.4.0 with installation schema 1. Its owned service, launcher, installation files and stale V2 socket/lock were inspected before explicit retirement. The verified 0.5.0 binary created a fresh schema 2 installation with the existing session resource limits. The old installation's ordinary update command was not used across the schema boundary.

The selected release, installed binary hash, enabled/active user service and local Health passed. The actual noninteractive SSH `session-proxy` route then passed Health and exact Unicode/whitespace Share/Fetch; only the synthetic probe item was deleted. No OS clipboard was used by this CLI probe. SSH keys, host trust and forced-command restrictions were unchanged.

The final scoped audit found only the 0.5.0 release in the managed installation and the session socket/lock in its runtime directory. The obsolete 0.4 migration recovery directory was inspected, checked for active references and permanently deleted. Temporary staging files were removed, and the service remained enabled, active and reachable.

## Completed Windows and Mac Migration

Windows installed the signed 0.5.0 per-user package and bundled headless CLI. Fresh schema 3 `settings.json` settings selected the session server; installed CLI Health passed. The old settings file, lock and schema backup, old startup registration and stale PATH entry pointing to an absent legacy installation were permanently removed. Other PATH entries and its registry value type were preserved. The final audit found one installed 0.5.0 desktop process and only the fresh settings file and its lock in the settings directory.

The first Windows panel still displayed 0.4.3 despite 0.5.0 files on disk: the existing process had old code loaded. Quitting that process and relaunching the installed app resolved the mismatch. A subsequent launch through the actual Start menu shortcut showed 0.5.0, the saved server and shared history. The underlying reason that the old process survived replacement was not established; this observation does not establish a package defect. Verify the running panel after replacement rather than relying on disk version metadata alone; see [troubleshooting](../troubleshooting.md#updates-and-startup).

The Apple Silicon Mac previously ran 0.4.3. Its transferred 0.5.0 archive matched the independently verified hash; native archive boundaries, arm64 app/CLI identity and code signatures passed before replacement. Fresh schema 3 settings retained the reviewed SSH connection options, including strict host-key verification, and the new CLI passed Health before installation. The old process was stopped, the application bundle replaced and installed Health repeated successfully. Old settings, the old LaunchAgent and the inspected recovery directory were permanently removed. Staging files were deleted after retaining the verification receipt. One installed 0.5.0 desktop process and its session SSH child remained; the owner confirmed the new panel and shared history.

Both clients received fresh device identities and defaults rather than imported configuration. Start at login was off on both desktops at the final checks. The owner chose to configure it independently; later startup settings and login behavior are not part of this acceptance.

## Owner-Confirmed Native Checks

| Check | Windows | Apple Silicon Mac |
| --- | --- | --- |
| Installed panel shows 0.5.0 and shared history | Passed, including Start menu launch | Passed after installed-app launch |
| Share text from the native clipboard | Passed; item appeared in history | Passed; item appeared in Windows history |
| Copy selected text and paste into a native application | Passed in Notepad after replacing the clipboard contents | Passed in TextEdit |
| Share an image from the native clipboard | Passed; image appeared in history | Passed from Preview; image appeared in Windows history |
| Copy selected image and paste into a native application | Passed in Paint after replacing the clipboard contents | Passed with Preview's New from Clipboard |

These confirmations complete the bounded Windows/Mac text and image transfer checks for this migration. They supplement the automated and package evidence above; they do not rerun earlier broad acceptance campaigns.

Ignored local receipts remain under `target/release-0.5.0/`, including server, Windows and Mac cutover checks. They are stage-specific snapshots: the initial Windows receipt predates GUI launch and old-settings deletion, and the Mac cutover receipt predates the owner-confirmed GUI checks. The completed results above include the later cleanup audits and owner confirmations.

## Remaining Evidence Limits

This migration did not exercise 0.5.0 login startup, native shortcut activation, built-in updater replacement, browser-download Gatekeeper behavior, cancellation or uncertain-share recovery. Earlier native reports retain their exact versions and scope; current automated coverage remains separate from live evidence. Linux desktop packaging is not covered.

Permanent removal of the inspected old installations/settings and recovery material was an explicit owner choice for these systems. The general [fresh cutover guide](../compatibility-and-cutover.md) retains its conservative preservation procedure for other installations. No new release, SSH key change or forced-command change was needed to complete the migration.

## Update Triggers

- A recorded source, package, publication result or evidence boundary needs correction.
- A later owner-confirmed native result extends the recorded live-migration scope.
- A later release supersedes latest; preserve these exact identities.

## Related Docs

- [Release operations](../releasing.md), [testing](../testing.md), [fresh cutover](../compatibility-and-cutover.md).
- [Desktop 0.4.3 acceptance](release-0.4.3-acceptance.md), [historical Linux cutover](linux-cutover-acceptance.md).
