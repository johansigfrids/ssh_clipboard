# 0.6.0 File Transfer Release Acceptance

> Historical record, 2026-10-23. Results apply to the exact source and packages below. Use [the docs index](../index.md) for current guides.

## Purpose

Record protected desktop/server 0.6.0 staging and independent package verification for file copying. Both releases remain drafts; publication requires separate owner approval. Installed applications and the managed server have not been replaced.

## Key Files

- `.github/workflows/desktop-release.yml`, `.github/workflows/server-release.yml`: protected signing and draft admission.
- `desktop/release-notes/0.6.0.md`: committed desktop notes.
- `docs/update-signing-public-key.pub`: independently pinned production trust.
- [File transfers](../file-transfers.md), [release operations](../releasing.md), [source/native acceptance](file-transfer-source-acceptance.md).

## Source and Automated Gates

File transfers were committed as `016eb2d29e5153ec29a8dfca2240e45c75c0983c`, with all ten [implementation CI jobs](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39946368670) passing. Release preparation is `eee590bd82fe0d8737ec47b4020d899179e3817e`; all three signed packages record this source, version 0.6.0 and the release profile.

The [release preparation CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39949131114) passed core checks on Windows/Mac/Linux, desktop checks on all three platforms and disposable server maintenance. Its browser job and one isolated retry timed out during slow Ubuntu dependency downloads before executing tests. Follow-up `44877dcea51f4867699e51c3b855a1fbeb3a87bb` changes only `.github/workflows/ci.yml` and `docs/ci.md`, increasing the browser job budget to twenty minutes while retaining scenario deadlines and zero retries. [Full follow-up CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39951965183) is in progress. Signed source and package bytes remain unchanged.

[Desktop staging](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39949145227) and [server staging](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39949148520) passed credential-free validation and protected signing after explicit owner authorization. Both environments retain required owner reviews and master-only rules. Public trust matches the reviewed source; key ID is `08c20604ca8fb748`.

## Exact Packages and Independent Verification

| Platform | Package bytes | SHA-256 |
| --- | ---: | --- |
| Windows x64 | 4,622,439 | `9322a12e1ad53b1e1f970df81d2e6b84c24e607410fdb65afd42378a7f9371b2` |
| Apple Silicon Mac | 7,619,304 | `fbf97844f73202ed0ab8b06700b44cc397302fb49ad522cf75e453eba777c82c` |
| Linux x64 server | 5,908,472 | `6077c01ea870fa3b9221c1f2d4814f1cc9bf5275bf5f78d9fc9ac72d6c5af75c` |

All twelve desktop and five server assets were downloaded from their drafts and compared with the corresponding protected workflow artifacts. Checks cover exact source/version/platform, release profile, package sizes/hashes/checksums, candidates and authenticated signatures. Both desktop signatures and the server signature passed offline verification against the repository-pinned public key.

Desktop admission checks clean production source, matching bundled CLI versions, committed notes and the combined keyed update feed. The Mac receipt confirms arm64 app/CLI identity, archive inspection and native code-signature verification. The server draft description was shortened to version-specific user-facing notes without changing signed assets.

The exact verified server binary passed a disposable WSL daemon/proxy fixture: version, Health, file Share/List/raw Fetch, overlapping-root deduplication, arbitrary binary/hidden/zero-byte files, empty folders, portable names/collisions, epoch-guarded extraction, overwrite refusal, wrong-epoch refusal, Delete, text exact-byte transfer and settings isolation. The process stopped and its temporary directory was removed. This used a synthetic SSH shim and no systemd service or live host.

Ignored downloads, workflow artifacts, verification scripts and receipts remain under `target/release-0.6.0/`.

## Native Evidence and Limits

[Source/native acceptance](file-transfer-source-acceptance.md) records Explorer/Finder paste in all four Windows/Mac directions, hash/tree comparisons, app Share/Copy, shortcuts, destination conflicts and renamed entries, and repeated paste after hide, Quit and reopening. Those isolated source builds and their clipboard-backed retained files were not replaced during release staging.

The exact 0.6.0 desktop packages have not been installed, and this checkpoint does not establish live SSH-connected file transfers between the installed desktops, production server upgrade, login/reboot, browser-download Gatekeeper behavior or built-in updater replacement. Existing package, service recovery and installer evidence retains its original version and scope. No additional broad manual campaign was performed.

## Compatibility and Publication

0.6.0 requires session version 2 on both clients and server; 0.5.0 peers use version 1 and are incompatible. Existing 0.5 settings schema 3, device identities and managed-server schema 2 are preserved. Coordinate upgrades; the earlier fresh 0.5 migration is not required again.

At this checkpoint, `desktop-v0.6.0` and `server-v0.6.0` are unpublished drafts. Desktop 0.5.0 remains repository latest. Server publication must explicitly retain `--latest=false`; only the reviewed desktop release carrying the combined feed may become latest. Public delivery has not yet been verified.

## Update Triggers

- Completion of the remaining CI check, separate publication approval or exact-byte public delivery verification.
- New installed/native evidence for these exact packages; preserve the source and hashes above.

## Related Docs

- [Release operations](../releasing.md), [file transfers](../file-transfers.md), [compatibility](../compatibility-and-cutover.md).
- [Source/native acceptance](file-transfer-source-acceptance.md), [previous release](release-0.5.0-acceptance.md).
