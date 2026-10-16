# Releasing

## Purpose
Describe legacy releases, staged signed desktop releases, key operations, and verified recovery.

## Key Files
- `.github/workflows/release.yml`
- `.github/workflows/desktop-release.yml`
- `desktop/scripts/build-installed.mjs`
- `Cargo.toml`
- `CHANGELOG.md` (if used)

## Legacy CLI/Agent Release Steps
1. Ensure `master` is green in CI.
2. Update docs for the release:
   - Update `CHANGELOG.md` with the release notes.
   - Verify `README.md` is accurate for installation and the common setup.
3. Verify `.github/workflows/release.yml` is up to date with current release outputs:
   - Build matrix entries (platforms/targets and agent vs server variants).
   - Artifact names and packaging format (`.zip`/`.tar.gz`) match docs and expected release assets.
   - Added/removed binaries are reflected in packaging commands.
4. Bump the version in `Cargo.toml` and commit the change.
5. Decide the next version (e.g., `0.2.0`) and tag with `v` prefix:
   - `git tag v0.2.0`
   - `git push origin v0.2.0`
6. GitHub Actions runs `release.yml` and publishes artifacts + checksums.

## Validate Artifacts
- Download the artifacts from the GitHub Release page.
- macOS publishes two agent artifacts:
  - `ssh_clipboard-macos-x86_64-agent` (Intel)
  - `ssh_clipboard-macos-aarch64-agent` (Apple Silicon)
- Smoke test:
  - `ssh_clipboard --help`
  - `ssh_clipboard proxy --help`
  - `ssh_clipboard daemon --help` (Linux)
  - `ssh_clipboard agent --help` (Windows/macOS/Linux agent builds)

## Signed Desktop Drafts

The current rework candidate is **0.4.0**. Refreshed automated evidence and remaining review/merge/signing/platform gates are tracked in [release readiness](release-readiness.md). The versioned unreleased notes do not imply publication or authorize a tag.

The new workflow does not run on legacy `v*` tags. It accepts manual dispatch from `master`, validates the exact commit/core version, and requires the `desktop-release` environment before a Windows x64 signed build. It creates a **draft** `desktop-v<version>` with package/signature/checksum, candidate JSON, keyed feed, and evidence. The owner has configured production signing inputs; both 0.4.0 protected builds have now passed and created drafts. Independent package verification and Windows production installation/text paste pass; see [exact-artifact acceptance](release-0.4.0-acceptance.md). Installed/native acceptance is still required; source/cryptographic and debug package proofs do not waive it.

Before first use, configure that environment with required reviewers and a master-only deployment rule. Add `DESKTOP_UPDATE_PUBLIC_KEY` as an environment variable and `DESKTOP_SIGNING_PRIVATE_KEY` / `DESKTOP_SIGNING_PASSWORD` as environment secrets. Normal PR checks receive none of these secrets. Protect the source branch and review release workflow/build/dependency changes before approval. Treat build scripts on an approved release ref as trusted code: the signing step builds from that ref and has the signing credentials.

Generate a dedicated key locally using the existing CLI, outside the checkout: `npm run tauri -- signer generate --write-keys <secure-path>` from `desktop`. The owner completed this on 2026-10-14 and confirmed saving the encrypted private key/password in the password manager. Both protected GitHub environments now hold the working signing inputs. No additional separate encrypted backup is required by this project. Do not paste private keys/passwords into issues, chat, docs, or source. The public `.pub` value is safe to configure/inspect. Synthetic test keys remain deliberately disposable.

Read-only verification on 2026-10-14 confirmed required owner approval (self-review permitted), one selected-branch rule for `master`, both expected secret names in each environment, and desktop/server public-key variables matching the locally generated public file. This verifies configuration metadata and public trust consistency; private key/password values were never read. Both protected production signing jobs subsequently succeeded on 2026-10-16. The selected identity is shared across the desktop/server environments, with separate secret scopes and authenticated asset names.

For local staging, use `npm run build:installed -- --version <core-version> --public-key-file <public-key>` with protected signing environment inputs. No release version is chosen automatically. The helper bundles a CLI matching that core version, signs the renamed exact package with authenticated version/filename, and verifies it before writing feed/checksum evidence. Do not install artifacts under `TEST-only-*` as production builds.

Desktop production staging now requires clean Git state before and after building, plus source-SHA parity with the workflow. Debug builds require explicit `--test-build`. Evidence records source, dirty state, profile, package bytes/hash and matching CLI identity. The draft handoff rejects test/debug/dirty/wrong-source/altered-byte evidence and selects six canonical assets, rather than counting files matching a broad pattern. Inert-adapter tests execute that actual workflow block; the synthetic installer smoke checks evidence against the real verified package. Those tests alone do not prove protected production signing; the later approved 0.4.0 runs provide that separate evidence. See [admission and prevention guidance](release-readiness.md#desktop-draft-admission).

Review a complete draft before publishing: exact source SHA/version, expected platform, independent public key, signatures, checksums, CLI parity, notes, and native acceptance results. The offline `verify_desktop_update` tool takes public-key file, current version, candidate JSON, and package; verify against your independently retained public key. Downloaded public-key text alone does not establish initial trust. Publication is a separate explicitly authorized step, after clean-install/old-to-new upgrade, startup/config preservation, cancellation/interruption, and platform gates. A draft is not discoverable by the stable feed.

## Signed Server Drafts

`server-release.yml` is a separate manual, master-only workflow. It validates the immutable commit/version and headless tests without credentials, then requires the protected `server-release` environment before packaging. Configure required reviewers, master-only deployment, `SERVER_UPDATE_PUBLIC_KEY` as an environment variable and `SERVER_SIGNING_PRIVATE_KEY` / `SERVER_SIGNING_PASSWORD` as environment secrets. The password manager remains the owner recovery location; no additional backup folder is required. Review the chosen public key independently before initializing a server installation. Server and desktop credential scopes are separate even if the owner deliberately chooses the same signing identity; authenticated asset names prevent cross-use.

The packaging job currently builds only Linux x86_64 on Ubuntu 24.04, with pinned Rust/actions and committed dependency locks. This defines the build environment, not proven compatibility with every older distribution. Native runtime/systemd acceptance is required before shipping; arm64 policy support is not an arm64 release claim. The approved 0.4.0 production workflow passed on GitHub; its downloaded signed artifact also passed isolated SSH, released-client and runtime user-systemd acceptance. Persistent installation/login and public delivery remain separate.

Local staging uses `npm run build:server -- --version <core-version> --public-key-file <trusted.pub>` from `desktop`, with the protected `TAURI_SIGNING_PRIVATE_KEY` / `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` inputs. The helper supports native Linux and the existing Ubuntu WSL toolchain on Windows. Production staging requires a clean checkout and matching stable core version; debug staging requires explicit `--test-build`. Private keys stored as files must be outside the checkout. Build/probe subprocesses receive no signing credentials; only the captured signer invocation does.

The helper builds headless, checks the binary's reported version, copies it to the canonical `SSHClipboardServer-stable-linux-<arch>-<version>` filename, signs those exact bytes with authenticated version/filename and runs the concrete server verifier before writing checksums/candidate/build evidence. The evidence records source SHA, profile, platform, key ID, size/hash, and `installed:false` / `published:false`. It never installs or restarts a service. Candidate metadata is for review; package signature/trusted identity, rather than unauthenticated candidate text/checksums, establish admission.

The workflow creates only a **draft** `server-v<version>`, with the raw binary, `.sig`, `.sha256`, candidate and evidence. It refuses an existing release and never overwrites assets or publishes. The maintenance downloader requests this exact tag/version. Before separately authorized publication, verify independent public trust, source/target/runtime, signatures, retained executable and real service rollback/recovery on a disposable installation. Existing legacy `v*` releases remain a separate pipeline. Server key loss/compromise needs an independently trusted management repair; there is no automatic feed-driven server trust rotation.

`npm run test:server-package` generates disposable synthetic trust, invokes the real helper in test-only debug mode, and rejects a mismatched public key before candidate/success evidence can be written. It retains public verification/evidence files and removes private test material without installing anything. It passed on Windows/Ubuntu WSL on 2026-10-14. `npm run test:server-maintenance` separately exercises the signed executable lifecycle with fake systemctl and real daemon/proxy processes. Neither substitutes for actual user-systemd or live migration acceptance.

The explicit `test:server-maintenance -- --ssh-target <authorized-host>` mode now also passes real user-systemd acceptance with signed debug fixtures, including crash-loop rate-limit rollback and full cleanup. It uses runtime-only files and preserves the installed service/SSH route. The exact optimized 0.4.0 production artifact subsequently passed its own isolated runtime checks. Persistent install/login remains open; see [service evidence](server-maintenance.md#real-user-systemd-acceptance).

## Key Rotation and Recovery

GitHub's `latest` release is repository-wide: the desktop keyed-feed URL depends on it. Server drafts explicitly use `--latest=false`; when separately authorized to publish one, use `gh release edit server-v<version> --draft=false --latest=false --repo johansigfrids/ssh_clipboard`. Keep that flag at publication rather than relying on a draft's saved state or the web UI default. Legacy `v*` publishing also sets `make_latest: false`; download those archives through their explicit release tag. Only a reviewed desktop release carrying the keyed feeds may become latest. Publishing another release type as latest would make desktop feed assets unavailable. No publication command has been run as part of setup.

Each binary follows `desktop-stable-<public-key-fingerprint>.json`. To rotate, stage a transition embedding the new public key/feed while signing it with the old key; pass the old public key with `--transition-from-public-key-file`. Verify both package trust and the installed transition before issuing packages signed only by the new key. Keep the old-key feed serving that transition for users who skip releases. Every later stable release must carry the retained old-key feed(s), because GitHub's `latest` asset URL follows the newest stable release. Do not point an old-key feed at a package it cannot verify. The initial automated draft workflow does not perform rotation or copy retained transition feeds: prepare/review those additions explicitly before publication.

The synthetic transition test proves signature/key selection, not an installed transition. Rehearse the full installed path with test trust and backups before rotating production trust. If the current private key is lost without a backup, an unsigned feed cannot introduce a replacement key. Provide a trusted, independently verified reinstall and preserve configuration. If a key is compromised, old clients cannot distinguish signatures made by the attacker; stop normal publication and use a separately trusted recovery/reinstall channel. OS publisher signing remains independent from updater signatures.

Do not delete published tags or rewrite/replace signed release assets as rollback. Publish a newly versioned corrective release. A stable feed may temporarily stop offering a bad update while investigation proceeds, but must not bypass signature or downgrade checks. For urgent manual recovery, independently verify a known-good installer and back up configuration before running it; describe any explicit downgrade separately from the normal updater. Preserve the old app/configuration during the migration support window. Do not promise automatic desktop rollback: platform installers have not demonstrated it here, and interrupted replacement requires checking the installed app/version.

## Update Triggers
- Changes to release packaging, platforms, or artifact naming.
- Changes to versioning strategy or CI release triggers.

## Related Docs
- `docs/ci.md`
- `docs/testing.md`
