# Desktop 0.4.2 Publication Acceptance

> Historical record, archived 2026-10-18. Version, latest-release and pending-gate statements below belong to the recorded source/session. Use [the docs index](../index.md) for current guides; preserve exact identities and unobserved limits when adding corrections.

## Purpose

Record the owner-authorized publication of the verified Windows x64 and Apple Silicon packages, independently of native migration/update evidence and later documentation commits.

## Key Files

- `.github/workflows/desktop-release.yml`, `desktop/scripts/desktop-draft.mjs`: protected exact-source multi-platform draft staging.
- `docs/update-signing-public-key.pub`: independently pinned production verification key.
- [docs/history/macos-cutover.md](macos-cutover.md), [docs/history/macos-updater-acceptance.md](macos-updater-acceptance.md): completed Mac migration, installed update and cleanup boundaries.
- [docs/releasing.md](../releasing.md), `CHANGELOG.md`, [README.md](../../README.md): publication policy and current downloads.

## Source and Merge Gates

The signed package source is `b465c4407805f4feb8d4f58cb9d28e209e26ed7e`, staged by [protected workflow 38928161438](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38928161438) after owner environment approval and all five validation/package/draft jobs passed. Production public trust remains `08c20604ca8fb748`; the SHA-256 of trimmed public-key text is `08c20604ca8fb74860ff1441426e3aa1d85f2ebf6acdbf44ef890d077bdcaab7`.

The owner explicitly requested merging [PR #9](https://github.com/johansigfrids/ssh_clipboard/pull/9) once CI passed, then publishing 0.4.2. All seven jobs passed in [CI run 38943790498](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38943790498) for head `dce532778ceb5ddcd8d8dec0afa727815f3e54e0`; the normal merge is `7b8f91dbb44ec7665585461b4a272077c939c16a`. No admin bypass or unverified head was used. Acceptance/test/docs commits do not change signed package source.

## Exact Packages

| Platform | Package bytes | SHA-256 |
| --- | ---: | --- |
| Windows x64 | 4,855,403 | `a34fc432ecfa1452f29943f0e356e89cc31b94d2f2cb95977117b9bf1ae0f7cd` |
| Apple Silicon Mac | 8,256,667 | `e77a028b48ed021a39ff436a832683785aefb86488fc722f8aafb6bf530897b1` |

Fresh authenticated draft downloads matched all twelve original review files byte-for-byte. Both platform packages passed the offline signature/authenticated release-identity verifier using the source-pinned public key, clean release evidence, matching CLI/source/version, checksums, candidate/signature associations and Mac native inspection receipt. The combined keyed feed contains exactly Windows x64 and Apple Silicon entries. No package was rebuilt, re-signed, replaced or reinstalled for publication.

## Publication and Public Delivery

[Desktop 0.4.2](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.4.2) was published as repository latest at `2026-10-16T18:05:53Z`. The public tag resolves to the original signed source. The release-page notes identify supported packages, acceptance and the Mac ad-hoc signature without changing reviewed assets.

Anonymous HTTPS requests, without a GitHub token, downloaded all twelve canonical assets. Their bytes, sizes and hashes exactly match the fresh verified draft and original review files; both public packages again pass independent signature/identity verification. The repository-wide latest keyed-feed URL serves the exact reviewed combined feed. Public API metadata confirms latest, non-draft, non-prerelease and the expected release/tag identity.

The published Linux server 0.4.0 and superseded held desktop 0.4.0 draft retain their prior identities, states and asset hashes. Earlier desktop 0.4.1 assets remain unchanged. Publication does not install/restart Windows or Mac, change either shared configuration/startup registration, or touch the live Linux service.

Ignored local evidence is under `desktop/artifacts/release-publication-0.4.2-uUi3FG/`, with separate draft/public downloads and verification receipts. Operator state is `target/release-0.4.2/state.json`. No private signing input was read or transferred.

## Evidence Limits

The completed [clean Mac migration](macos-cutover.md) proves production text share/copy, saved settings, actual login and panel shortcut; the separate [native updater rehearsal](macos-updater-acceptance.md) proves actual replacement/restart with disposable trust, private loopback delivery, retained test settings and complete cleanup. Anonymous public delivery is now proven independently. These do not claim native Mac image paste, browser-download Gatekeeper approval, interrupted replacement, Mac native key rotation or Linux desktop installation. Windows updater/fault/rotation acceptance remains its separately recorded evidence. The synthetic 0.4.3 test package is not a production release.

## Update Triggers

- A publication/delivery result, source identity or remaining platform boundary changes.
- A future desktop release changes latest, public trust or the combined platform feed.

## Related Docs

- [Release readiness](release-readiness.md), [release operations](../releasing.md).
- [Mac migration](macos-cutover.md), [Mac updater acceptance](macos-updater-acceptance.md).
- [Architecture](../../ARCHITECTURE.md), [implementation plan](implementation-plan.md).
