# Releasing

## Purpose
Describe legacy releases, staged signed desktop releases, key operations, and verified recovery.

## Key Files
- `.github/workflows/desktop-release.yml`
- `desktop/scripts/build-installed.mjs`
- `desktop/scripts/release-notes.mjs`, `desktop/release-notes/<version>.md`
- `Cargo.toml`
- `CHANGELOG.md` (release history)

## Retired Legacy Distribution

The old automatic `v*` CLI/agent/server release publisher is removed. The legacy tray agent is retired from default builds and future distribution; its explicit compatibility feature and published v0.3.0 archives remain through the 0.4 compatibility period. See [legacy retirement](legacy-retirement.md).

Do not create a legacy tag to publish a current desktop or server. Use the existing protected signed workflows, independent artifact admission and separate publication authorization below. Do not delete/rewrite historical tags or assets. An exceptional new legacy release would require a separate owner decision and reviewed staging workflow.

## Validate Artifacts

Select assets by the explicit desktop/server tag and reviewed version, then verify exact bytes, authenticated release identity, source evidence, checksums and matching CLI. Use independent public trust before executing anything. [Desktop staging](#signed-desktop-drafts) and [server staging](#signed-server-drafts) own canonical asset layouts; [release history](history/release-readiness.md) retains older layouts and exact accepted identities.

Current desktop targets are Windows x64 and Apple Silicon; there is no current Intel Mac package. The older Intel/Apple Silicon agent archives remain frozen under v0.3.0. For those archives only, verify the expected compatibility command surface, including `agent --help`. For current desktop/CLI and Linux server packages, check `--help`, matching versions, and the relevant shelf/proxy commands. Compilation or CLI help alone does not prove native paste, installed startup or replacement.

## Signed Desktop Drafts

The [release evidence overview](history/release-readiness.md) owns published-version and installed-acceptance history. Retain the superseded desktop 0.4.0 draft without publishing or replacing its bytes. Future desktop publication requires separate authorization after exact-package review; server releases must preserve desktop latest/feed selection.

The workflow does not run on legacy `v*` tags. Manual dispatch from `master` validates the exact commit/core version without credentials on Windows and macOS. Both Windows x64 and Apple Silicon packaging jobs require the `desktop-release` environment. Each signs/verifies its native package and uploads public review artifacts; a separate credential-free admission job combines both into one `desktop-v<version>` draft and keyed feed. Exact production/native/public-delivery results are retained in [0.4.3 acceptance](history/release-0.4.3-acceptance.md), [0.4.2 acceptance](history/release-0.4.2-acceptance.md), [Mac cutover](history/macos-cutover.md) and [Mac updater acceptance](history/macos-updater-acceptance.md).

Before first use, configure that environment with required reviewers and a master-only deployment rule. Add `DESKTOP_UPDATE_PUBLIC_KEY` as an environment variable and `DESKTOP_SIGNING_PRIVATE_KEY` / `DESKTOP_SIGNING_PASSWORD` as environment secrets. Normal PR checks receive none of these secrets. Protect the source branch and review release workflow/build/dependency changes before approval. Treat build scripts on an approved release ref as trusted code: the signing step builds from that ref and has the signing credentials.

Generate a dedicated key locally using the existing CLI, outside the checkout: `npm run tauri -- signer generate --write-keys <secure-path>` from `desktop`. The owner completed this on 2026-10-14 and confirmed saving the encrypted private key/password in the password manager. Both protected GitHub environments now hold the working signing inputs. No additional separate encrypted backup is required by this project. Do not paste private keys/passwords into issues, chat, docs, or source. The public `.pub` value is safe to configure/inspect. Synthetic test keys remain deliberately disposable.

Read-only verification on 2026-10-14 confirmed required owner approval (self-review permitted), one selected-branch rule for `master`, both expected secret names in each environment, and desktop/server public-key variables matching the locally generated public file. This verifies configuration metadata and public trust consistency; private key/password values were never read. Both protected production signing jobs subsequently succeeded on 2026-10-16. The selected identity is shared across the desktop/server environments, with separate secret scopes and authenticated asset names.

For local staging, use `npm run build:installed -- --version <core-version> --public-key-file <public-key>` with protected signing environment inputs. No release version is chosen automatically. The helper bundles a CLI matching that core version, signs the renamed exact package with authenticated version/filename, and verifies it before writing feed/checksum evidence. Do not install artifacts under `TEST-only-*` as production builds.

Desktop production staging requires clean Git state before and after building, plus source-SHA parity with the workflow. Debug builds require explicit `--test-build`. Evidence records source, dirty state, profile, package bytes/hash and matching CLI identity. The `desktop-draft.mjs` handoff requires both platforms, verifies exact bytes/checksums and matching candidate/feed/signature metadata, and pins signing/embedded trust to the public key in the reviewed source. It merges both entries into one keyed feed; a Mac-only draft cannot displace Windows updates. It refuses existing signed releases. Rotation and retained old-key feeds remain a separately prepared operation.

The combined draft has twelve canonical assets: each platform's package, `.sig`, `.sha256`, `candidate-<platform>.json` and `build-evidence-<platform>.json`; the Mac archive inspection receipt; and one `desktop-stable-<key-id>.json`. Historical Windows-only releases retain their original six-asset layout. Inert-adapter tests execute the actual handoff function, including partial-platform refusal and feed agreement; package smoke checks use the real builder. These checks do not replace protected signing or native acceptance. See [admission and prevention guidance](history/release-readiness.md#desktop-draft-admission).

Review a complete draft before publishing: exact source SHA/version, expected platform, independent public key, signatures, checksums, CLI parity, notes, and native acceptance results. The offline `verify_desktop_update` tool takes public-key file, current version, candidate JSON, and package; verify against your independently retained public key. Downloaded public-key text alone does not establish initial trust. Publication is a separate explicitly authorized step after reviewing the applicable automated/artifact checks and relevant native acceptance. The [implementation plan](history/implementation-plan.md) records deferred platforms and non-blocking omitted scenarios; do not reopen a broad manual campaign for unrelated changes. A draft is not discoverable by the stable feed.

## Desktop Release Notes

For each desktop version, commit `desktop/release-notes/<version>.md` alongside the core version change. Use an exact first heading and plain user-facing bullets, for example:

```markdown
# 1.2.3

- Describe a concrete user-visible change.
- Describe another change in this version.
```

Run `node desktop/scripts/release-notes.mjs <version>` from the repository root before staging. This preflight needs no signing inputs. The protected workflow runs it before compilation, and the installed builder repeats it before accessing signing inputs or creating a stage. A missing file, mismatched/non-stable version, empty body, extra headings/prose, draft/publication-pending wording, or text above 16 KiB of UTF-8 is rejected. LF and CRLF normalize to the same text. Keep bullets concise and plain; the app displays recognized short notes as escaped text, not arbitrary Markdown or HTML.

Production always selects the exact committed version file and never falls back to `CHANGELOG.md`. Explicit `--notes-file` overrides are reserved for `--test-build` packages and obey the same version/format/budget rules. The disposable Windows/Mac updater helpers generate versioned fixture notes accordingly.

Both admitted platform feeds must match the reviewed source notes. The handoff revalidates that agreement and writes the same text to the desktop draft description's `--notes-file`. This description input is not an extra release asset; the twelve canonical assets and package/signature admission remain unchanged. Server release notes and tooling are separate.

The source's 0.4.2 notes file does not change the published 0.4.2 release or feed. Apply this contract when staging a newly versioned release; publication and any change to existing release metadata still require their separate authorization. Do not rewrite signed assets to fix historical notes.

## Signed Server Drafts

`server-release.yml` is a separate manual, master-only workflow. It validates the immutable commit/version and headless tests without credentials, then requires the protected `server-release` environment before packaging. Configure required reviewers, master-only deployment, `SERVER_UPDATE_PUBLIC_KEY` as an environment variable and `SERVER_SIGNING_PRIVATE_KEY` / `SERVER_SIGNING_PASSWORD` as environment secrets. The password manager remains the owner recovery location; no additional backup folder is required. Review the chosen public key independently before initializing a server installation. Server and desktop credential scopes are separate even if the owner deliberately chooses the same signing identity; authenticated asset names prevent cross-use.

The packaging job currently builds only Linux x86_64 on Ubuntu 24.04, with pinned Rust/actions and committed dependency locks. This defines the build environment, not proven compatibility with every older distribution. Native runtime/systemd acceptance is required before shipping; arm64 policy support is not an arm64 release claim. The approved 0.4.0 production workflow passed on GitHub; its downloaded signed artifact also passed isolated SSH, released-client and runtime user-systemd acceptance. The later [clean live cutover](history/linux-cutover-acceptance.md) passed persistent installation, actual SSH routes and legacy retirement. Anonymous public server delivery also passes; actual reboot/last-session logout remain separate.

Local staging uses `npm run build:server -- --version <core-version> --public-key-file <trusted.pub>` from `desktop/`, with the protected `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` inputs. The helper supports native Linux and the existing Ubuntu WSL toolchain on Windows. Production staging requires a clean checkout and matching stable core version; debug staging requires explicit `--test-build`. Private keys stored as files must be outside the checkout. Build/probe subprocesses receive no signing credentials; only the captured signer invocation does.

The helper builds headless, checks the binary's reported version, copies it to the canonical `SSHClipboardServer-stable-linux-<arch>-<version>` filename, signs those exact bytes with authenticated version/filename and runs the concrete server verifier before writing checksums/candidate/build evidence. The evidence records source SHA, profile, platform, key ID, size/hash, and `installed:false` / `published:false`. It never installs or restarts a service. Candidate metadata is for review; package signature/trusted identity, rather than unauthenticated candidate text/checksums, establish admission.

The workflow creates only a **draft** `server-v<version>`, with the raw binary, `.sig`, `.sha256`, candidate and evidence. It refuses an existing release and never overwrites assets or publishes. The maintenance downloader requests this exact tag/version. Before separately authorized publication, verify independent public trust, source/target/runtime, signatures, retained executable and real service rollback/recovery on a disposable installation. Existing legacy `v*` releases remain immutable historical downloads; their publisher is retired. Server key loss/compromise needs an independently trusted management repair; there is no automatic feed-driven server trust rotation.

`npm run test:server-package` generates disposable synthetic trust, invokes the real helper in test-only debug mode, and rejects a mismatched public key before candidate/success evidence can be written. It retains public verification/evidence files and removes private test material without installing anything. It passed on Windows/Ubuntu WSL on 2026-10-14. `npm run test:server-maintenance` separately exercises the signed executable lifecycle with fake systemctl and real daemon/proxy processes. Neither substitutes for actual user-systemd or live migration acceptance.

The explicit `test:server-maintenance -- --ssh-target <authorized-host>` mode now also passes real user-systemd acceptance with signed debug fixtures, including crash-loop rate-limit rollback and full cleanup. It uses runtime-only files and preserves the installed service/SSH route. The exact optimized 0.4.0 production artifact subsequently passed isolated runtime checks and the separately authorized persistent live cutover. See [service evidence](server-maintenance.md#real-user-systemd-acceptance) and [live acceptance](history/linux-cutover-acceptance.md) for their distinct boundaries.

## Key Rotation and Recovery

GitHub's `latest` release is repository-wide: the desktop keyed-feed URL depends on it. Server drafts explicitly use `--latest=false`; when separately authorized to publish one, use `gh release edit server-v<version> --draft=false --latest=false --repo johansigfrids/ssh_clipboard`. Keep that flag at publication rather than relying on a draft's saved state or the web UI default. The retired legacy `v*` publisher no longer runs; download historical archives through their explicit release tag. Any separately authorized exceptional legacy publication must still use `--latest=false`. Only a reviewed desktop release carrying the keyed feeds may become latest. Publishing another release type as latest would make desktop feed assets unavailable. The earlier desktop 0.4.1 and server 0.4.0 publications passed their separate approvals and latest/feed preservation checks; see [public server acceptance](history/linux-cutover-acceptance.md#server-publication). Desktop 0.4.3 is now latest with anonymous verification of both platform packages and the combined keyed feed; see [publication acceptance](history/release-0.4.3-acceptance.md).

Each binary follows `desktop-stable-<public-key-fingerprint>.json`. To rotate, stage a transition embedding the new public key/feed while signing it with the old key; pass the old public key with `--transition-from-public-key-file`. Verify both package trust and the installed transition before issuing packages signed only by the new key. Keep the old-key feed serving that transition for users who skip releases. Every later stable release must carry the retained old-key feed(s), because GitHub's `latest` asset URL follows the newest stable release. Do not point an old-key feed at a package it cannot verify. The initial automated draft workflow does not perform rotation or copy retained transition feeds: prepare/review those additions explicitly before publication.

The synthetic transition test proves signature/key selection, not an installed transition. Rehearse the full installed path with test trust and backups before rotating production trust. If the current private key is lost without a backup, an unsigned feed cannot introduce a replacement key. Provide a trusted, independently verified reinstall and preserve configuration. If a key is compromised, old clients cannot distinguish signatures made by the attacker; stop normal publication and use a separately trusted recovery/reinstall channel. OS publisher signing remains independent from updater signatures.

Do not delete published tags or rewrite/replace signed release assets as rollback. Publish a newly versioned corrective release. A stable feed may temporarily stop offering a bad update while investigation proceeds, but must not bypass signature or downgrade checks. For urgent manual recovery, independently verify a known-good installer and back up configuration before running it; describe any explicit downgrade separately from the normal updater. Preserve the old app/configuration during the migration support window. Do not promise automatic desktop rollback: platform installers have not demonstrated it here, and interrupted replacement requires checking the installed app/version.

## Update Triggers
- Changes to release packaging, platforms, or artifact naming.
- Changes to versioning strategy or CI release triggers.

## Related Docs
- [docs/ci.md](ci.md)
- [docs/testing.md](testing.md)
