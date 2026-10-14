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

The new workflow does not run on legacy `v*` tags. It accepts manual dispatch from `master`, validates the exact commit/core version, and requires the `desktop-release` environment before a Windows x64 signed build. It creates a **draft** `desktop-v<version>` with package/signature/checksum, candidate JSON, keyed feed, and evidence. No desktop production release or real signing credentials have been configured yet. Installed/native acceptance is still required; source/cryptographic and debug package proofs do not waive it.

Before first use, configure that environment with required reviewers and a master-only deployment rule. Add `DESKTOP_UPDATE_PUBLIC_KEY` as an environment variable and `DESKTOP_SIGNING_PRIVATE_KEY` / `DESKTOP_SIGNING_PASSWORD` as environment secrets. Normal PR checks receive none of these secrets. Protect the source branch and review release workflow/build/dependency changes before approval. Treat build scripts on an approved release ref as trusted code: the signing step builds from that ref and has the signing credentials.

Generate the dedicated key locally using the existing CLI, outside the checkout: `npm run tauri -- signer generate --write-keys <secure-path>` from `desktop`. Keep the encrypted private file and password in secure backed-up storage; keep a second recoverable backup under your control. Do not paste private keys/passwords into issues, chat, docs, or source. The public `.pub` value is safe to configure/inspect. Production key creation, backup location, and GitHub environment setup need the owner's action; synthetic test keys are deliberately disposable.

For local staging, use `npm run build:installed -- --version <core-version> --public-key-file <public-key>` with protected signing environment inputs. No release version is chosen automatically. The helper bundles a CLI matching that core version, signs the renamed exact package with authenticated version/filename, and verifies it before writing feed/checksum evidence. Do not install artifacts under `TEST-only-*` as production builds.

Review a complete draft before publishing: exact source SHA/version, expected platform, independent public key, signatures, checksums, CLI parity, notes, and native acceptance results. The offline `verify_desktop_update` tool takes public-key file, current version, candidate JSON, and package; verify against your independently retained public key. Downloaded public-key text alone does not establish initial trust. Publication is a separate explicitly authorized step, after clean-install/old-to-new upgrade, startup/config preservation, cancellation/interruption, and platform gates. A draft is not discoverable by the stable feed.

## Key Rotation and Recovery

Each binary follows `desktop-stable-<public-key-fingerprint>.json`. To rotate, stage a transition embedding the new public key/feed while signing it with the old key; pass the old public key with `--transition-from-public-key-file`. Verify both package trust and the installed transition before issuing packages signed only by the new key. Keep the old-key feed serving that transition for users who skip releases. Every later stable release must carry the retained old-key feed(s), because GitHub's `latest` asset URL follows the newest stable release. Do not point an old-key feed at a package it cannot verify. The initial automated draft workflow does not perform rotation or copy retained transition feeds: prepare/review those additions explicitly before publication.

The synthetic transition test proves signature/key selection, not an installed transition. Rehearse the full installed path with test trust and backups before rotating production trust. If the current private key is lost without a backup, an unsigned feed cannot introduce a replacement key. Provide a trusted, independently verified reinstall and preserve configuration. If a key is compromised, old clients cannot distinguish signatures made by the attacker; stop normal publication and use a separately trusted recovery/reinstall channel. OS publisher signing remains independent from updater signatures.

Do not delete published tags or rewrite/replace signed release assets as rollback. Publish a newly versioned corrective release. A stable feed may temporarily stop offering a bad update while investigation proceeds, but must not bypass signature or downgrade checks. For urgent manual recovery, independently verify a known-good installer and back up configuration before running it; describe any explicit downgrade separately from the normal updater. Preserve the old app/configuration during the migration support window. Do not promise automatic desktop rollback: platform installers have not demonstrated it here, and interrupted replacement requires checking the installed app/version.

## Update Triggers
- Changes to release packaging, platforms, or artifact naming.
- Changes to versioning strategy or CI release triggers.

## Related Docs
- `docs/ci.md`
- `docs/testing.md`
