# 0.6.0 File Transfer Release Acceptance

> Historical record, 2026-10-23. Results apply to the exact source and packages below. Use [the docs index](../index.md) for current guides.

## Purpose

Record protected desktop/server 0.6.0 staging, separately authorized publication and independent package/public-delivery verification for file copying. Installed applications and the managed server were unchanged at publication; subsequent authorized upgrades are recorded below.

## Key Files

- `.github/workflows/desktop-release.yml`, `.github/workflows/server-release.yml`: protected signing and draft admission.
- `desktop/release-notes/0.6.0.md`: committed desktop notes.
- `docs/update-signing-public-key.pub`: independently pinned production trust.
- [File transfers](../file-transfers.md), [release operations](../releasing.md), [source/native acceptance](file-transfer-source-acceptance.md).

## Source and Automated Gates

File transfers were committed as `016eb2d29e5153ec29a8dfca2240e45c75c0983c`, with all ten [implementation CI jobs](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39946368670) passing. Release preparation is `eee590bd82fe0d8737ec47b4020d899179e3817e`; all three signed packages record this source, version 0.6.0 and the release profile.

The [release preparation CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39949131114) passed core checks on Windows/Mac/Linux, desktop checks on all three platforms and disposable server maintenance. Its browser job and one isolated retry timed out during slow Ubuntu dependency downloads before executing tests. Follow-up `44877dcea51f4867699e51c3b855a1fbeb3a87bb` changes only `.github/workflows/ci.yml` and `docs/ci.md`, increasing the browser job budget to twenty minutes while retaining scenario deadlines and zero retries. [Full follow-up CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39951965183) passed all ten jobs, including the browser scenarios and final `CI complete` gate. Signed application source and package bytes remain unchanged.

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

At publication, the exact 0.6.0 desktop packages had not been installed, and that checkpoint did not establish live SSH-connected file transfers between the installed desktops, production server upgrade, login/reboot, browser-download Gatekeeper behavior or built-in updater replacement. Subsequent server and Windows installation evidence and the owner's Mac upgrade confirmation appear below. Existing package, service recovery and installer evidence retains its original version and scope. No additional broad manual campaign was performed.

## Compatibility and Publication

0.6.0 requires session version 2 on both clients and server; 0.5.0 peers use version 1 and are incompatible. Existing 0.5 settings schema 3, device identities and managed-server schema 2 are preserved. Coordinate upgrades; the earlier fresh 0.5 migration is not required again.

After reviewing the verification results and native evidence limits, the owner explicitly approved publishing both releases. [Server 0.6.0](https://github.com/johansigfrids/ssh_clipboard/releases/tag/server-v0.6.0) was published at 12:31:07 UTC with `--latest=false`; desktop 0.5.0 remained latest immediately afterward. [Desktop 0.6.0](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.6.0) was published at 12:31:09 UTC and became latest. Both published tags point to verified source `eee590bd82fe0d8737ec47b4020d899179e3817e`. Neither publication rebuilt or replaced signed assets.

Anonymous HTTPS downloads of all seventeen assets match the reviewed drafts byte-for-byte. Both releases are stable and non-draft; their version-specific descriptions match the reviewed notes. All three public package signatures pass verification against independently pinned trust again. The repository-wide latest keyed-feed URL serves the exact reviewed combined Windows x64/Apple Silicon 0.6.0 feed. `target/release-0.6.0/public-verification.json` retains the public verification receipt.

## Subsequent Installed Upgrades

Later on 2026-10-23, the owner authorized upgrading the existing managed server from 0.5.0. The exact signed Linux package above was verified before execution and activated using its 0.6.0 maintenance command. The enabled user service passed protocol health at 0.6.0, and the running executable hash matched the package. Settings and unit bytes were preserved, the previous managed release was retained and no recovery remained pending. Real SSH file Share/Fetch/extraction passed with multiple roots, overlapping-root deduplication, binary/hidden/Unicode/zero-byte files and an empty folder; extracted hashes matched. The epoch-guarded test item and disposable staging were removed. No OS clipboard was used for this server probe. Receipts are `target/release-0.6.0/live-server-{upgrade,verification,cleanup}.json`.

The Windows Start search investigation found that Codex's packaged process redirected its apparent LocalAppData executable path into a private MSIX `LocalCache` installation containing 0.5.0, while normal Start search launched a separate 0.4.3 installation. Both shortcuts targeted the same apparent path. The owner ran the verified 0.6.0 installer from ordinary File Explorer and confirmed that the same Start search result now shows 0.6.0 and the saved connection. Independent physical-file inspection verified both normal installed executables against extraction of that signed installer:

| Installed file | SHA-256 |
| --- | --- |
| `ssh_clipboard_desktop.exe` | `2cc2f9bfb3d252bbadf8397d33c56243ccfc47110d184e8bf3ce656b92ba7e25` |
| `ssh_clipboard.exe` | `603b2f14b0a8eb26881b73eaef0be4e4d756139eeebf3593653244bbb5b13d6e` |

The obsolete private 0.5.0 executable directory was renamed to a same-parent backup, and Codex then resolved the normal path to the verified 0.6.0 executable. The shared schema 3 Roaming settings remained byte-identical. A preceding WebView-cache experiment did not fix the cause and was fully reversed. Shortcuts were already correct. `target/release-0.6.0/windows-normal-install-repair.json` retains the repair receipt; see the [physical-path prevention guidance](../troubleshooting.md#updates-and-startup). This establishes a normal Windows installer replacement and Start search launch, not built-in updater replacement, login/reboot, or a new installed-desktop file-transfer campaign.

The owner subsequently confirmed completing the Mac upgrade to 0.6.0 on 2026-10-23. This records owner-confirmed completion of the coordinated Windows/Mac/server upgrade. The Mac installation method, installed hashes and post-upgrade native behavior were not independently checked in this session; the earlier package verification and source/native acceptance retain their original scope.

## Update Triggers

- Additional exact-byte public-delivery verification or release corrections.
- New installed/native evidence for these exact packages; preserve the source and hashes above.

## Related Docs

- [Release operations](../releasing.md), [file transfers](../file-transfers.md), [compatibility](../compatibility-and-cutover.md).
- [Source/native acceptance](file-transfer-source-acceptance.md), [previous release](release-0.5.0-acceptance.md).
