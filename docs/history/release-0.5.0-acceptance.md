# 0.5.0 Publication Verification

> Historical record, 2026-10-22. Results apply to the exact source and packages below. Use [the docs index](../index.md) for current guides.

## Purpose

Record the authorized desktop/server 0.5.0 publication, independent artifact checks and remaining native evidence limits. This release requires [fresh settings and server setup](../compatibility-and-cutover.md); publication does not migrate installed systems.

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

## Native and Rollout Limits

No 0.5.0 desktop package was installed or launched for native clipboard, shortcut, login or replacement acceptance. No fresh real-user-systemd or live SSH acceptance was performed for the exact 0.5.0 server package. Automated adapters, package inspection, disposable process checks and fake-systemctl maintenance are the evidence for this release; earlier native reports retain their original versions and scope.

No installed app, client settings, server installation, service, SSH route or forced-command key was changed. Preserve the old installation/settings, prepare a fresh schema 2 server installation, verify its session SSH route, then configure clients afresh with schema 3 `settings.json`. Do not use an old installation's ordinary update command for this transition.

## Update Triggers

- A recorded source, package, publication result or evidence boundary needs correction.
- A later release supersedes latest; preserve these exact identities.

## Related Docs

- [Release operations](../releasing.md), [testing](../testing.md), [fresh cutover](../compatibility-and-cutover.md).
- [Desktop 0.4.3 acceptance](release-0.4.3-acceptance.md), [historical Linux cutover](linux-cutover-acceptance.md).
