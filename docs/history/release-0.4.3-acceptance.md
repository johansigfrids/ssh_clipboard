# Desktop 0.4.3 Publication Acceptance

> Historical record, 2026-10-19. Results apply to the exact source and packages below. Use [the docs index](../index.md) for current guides.

## Purpose

Record the authorized desktop-only 0.4.3 release, exact signed packages, bounded Windows acceptance and anonymous public delivery.

## Key Files

- `.github/workflows/desktop-release.yml`, `desktop/scripts/desktop-draft.mjs`: protected multi-platform staging.
- `desktop/release-notes/0.4.3.md`: reviewed updater and release-page notes.
- `docs/update-signing-public-key.pub`: independently pinned production public trust.
- [Release operations](../releasing.md), [changelog](../../CHANGELOG.md), [downloads](../../README.md).

## Source and Automated Gates

[PR #27](https://github.com/johansigfrids/ssh_clipboard/pull/27) merged validated head `03d10359e6d530d5e9a7523ba47aabb1b94fc6a1` as `79205abfa5bf5459846cbe427d0ce13abc34f166`. All ten validation jobs and the final CI gate passed in [PR CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39217622437); [post-merge master CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39218186306) also passed.

The owner authorized signed draft staging. [Protected workflow 39218213869](https://github.com/johansigfrids/ssh_clipboard/actions/runs/39218213869) validated both native platforms without credentials, then passed both protected packaging jobs and combined draft admission. Before approving signing, the master-only environment rule, owner-review requirement and environment public-key agreement with source were checked. Public key ID remains `08c20604ca8fb748`.

## Exact Packages and Independent Admission

| Platform | Package bytes | SHA-256 |
| --- | ---: | --- |
| Windows x64 | 4,845,524 | `97d3eed061d349a66deef8753aa1f399ec0a4eda2b1c23a10fe43fed3adddf84` |
| Apple Silicon Mac | 8,250,790 | `e53d00d7afd5e4b412519e0eade2628864ac89805f15fb3b7703182af1f77aa9` |

Both downloaded native workflow artifacts and final draft packages passed the offline signature/authenticated release-identity verifier against the repository-pinned public key. All twelve canonical draft assets matched native workflow artifacts where applicable, clean release source, package hashes/sizes, matching `ssh_clipboard 0.4.3` CLI evidence, candidates, signatures and committed notes. The combined feed contains exactly Windows x64 and Apple Silicon entries. The Mac receipt confirms archive inspection, arm64 identity, native code signatures and CLI version.

## Owner-Confirmed Windows Acceptance

The owner ran the exact verified Windows installer and confirmed successful completion, normal tray-panel opening and retention of the existing connection. A harmless text Share appeared in history without a console flash. After replacing the local clipboard, selecting and copying that history item restored the original text in Notepad without a console flash. The owner accepted settings-draft behavior and explicitly ended further manual smoke testing.

This is bounded Windows text/settings acceptance. It does not add native image, login/reboot, installer-fault or key-rotation evidence, or a new installed Mac smoke test. Earlier [Mac migration](macos-cutover.md) and [Mac updater](macos-updater-acceptance.md) evidence retain their original source/version boundaries; automated and archive checks do not extend them to installed 0.4.3 behavior.

## Publication and Anonymous Delivery

After separate explicit owner approval, [desktop-v0.4.3](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.4.3) was published as repository latest on 2026-10-19. Its public tag resolves to signed source `79205abfa5bf5459846cbe427d0ce13abc34f166`. No package was rebuilt, re-signed or replaced for publication.

Anonymous HTTPS downloads of all twelve public assets matched the verified draft byte-for-byte. Both public packages again passed offline signature/release-identity verification. The repository-wide latest keyed-feed URL serves the exact reviewed combined 0.4.3 feed. Public metadata confirms latest, non-draft and non-prerelease status.

Linux server 0.4.0 retains its release/asset identities and publication state; no server package, installation or service was changed. The earlier desktop releases retain their assets. Local ignored receipts, draft/public downloads and CI metadata are under `target/release-0.4.3/`; later documentation commits do not change the signed source.

## Update Triggers

- A recorded source, package, publication result or evidence boundary needs correction.
- A later release supersedes latest; retain these exact historical identities.

## Related Docs

- [Release operations](../releasing.md), [testing](../testing.md), [0.4.2 acceptance](release-0.4.2-acceptance.md).
