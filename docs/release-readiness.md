# Rework Release Readiness

## Purpose

Keep the release candidate's automated evidence, prepared material, and remaining approval/platform gates in one place. Version 0.4.0 is chosen for review; preparation does not publish, merge, or replace the installed client/server.

## Key Files

- `CHANGELOG.md`: versioned 0.4.0 candidate notes; assign the actual publication date at release time.
- `.github/workflows/desktop-release.yml`, `server-release.yml`: master-only protected draft staging.
- `desktop/scripts/build-installed.mjs`, `release-source.mjs`: desktop source/package evidence.
- `desktop/tests/desktop-release.test.mjs`, `release-source.test.mjs`: source admission and actual draft-handoff refusal tests.
- [Release operations](releasing.md) and [compatibility/cutover](compatibility-and-cutover.md): trust, publication and live migration procedure.

## Candidate Scope

The rework candidate is **0.4.0** on `codex/ssh-clipboard-rework` in PR #1. The owner authorized choosing a sensible version and preparing the PR for merge on 2026-10-16. A minor pre-1.0 increment reflects the substantial desktop/history/update additions while the existing V2 route remains available; it does not claim the broader platform/operational maturity of 1.0. The shared core and both dependency lockfiles carry 0.4.0. The desktop frontend/prototype packages retain their internal 0.0.0 versions; installed staging overrides the app version from the matching core version.

The notes have a 0.4.0 section explicitly marked Unreleased, with no publication date. No tag or production release has been created. Synthetic updater versions and the pinned published v0.3.0 compatibility fixture remain unchanged; they are evidence inputs, not current application versions.

The initial protected workflows stage Windows x64 desktop and Linux x64 server packages. The existing legacy CLI/agent release pipeline remains separate. macOS/Linux desktop compilation is not native GUI/package acceptance, so do not advertise those rework installers as validated releases.

## Automated Evidence Refreshed on 2026-10-16

| Check | Result and boundary |
| --- | --- |
| Windows core | Formatting, required all-target agent lint, and the full suite pass. Three initial subprocess timing assertions failed under competing fixture families; shared two-slot admission fixed the contention without changing production deadlines or dropping assertions. |
| Desktop backend | Formatting, all-feature/all-target lint, history/migration/cancellation/frozen-recovery tests, update trust and framework handoff tests pass. Explicit live SSH tests remain opt-in; earlier remote/native results are recorded separately. |
| Frontend and release scripts | 56 rendered interactions, 13 Node tests, Svelte checks and production UI build pass. New tests execute the actual desktop workflow handoff using inert filesystem/API adapters, with no GitHub write or real signing. |
| Headless Linux | Existing WSL toolchain passes warnings-as-errors/all-target lint and full tests, including eight real private daemon/proxy cases for mixed modes, restart/no replay, selected exports and cleanup. This does not install a service. |
| Published compatibility | Seven archive/lifecycle guards and eight pinned actual v0.3.0 Linux protocol checks pass again, with private process/directory cleanup. |
| Synthetic Windows package | A disposable-key debug installer builds and verifies exact bytes/release identity. Its evidence matches source, explicit test/debug identity, size and SHA-256. Nothing was installed; the temporary private key was removed. This is not a production signed artifact. |
| Protected environment metadata | Reviewer gates, master-only branch rules, both expected secret names and matching desktop/server public-key fingerprints were rechecked. Private keys/passwords were not read; actual protected signing is still unproven. |

The synthetic package was built from the in-progress preparation tree and is explicitly marked `testOnly:true`, `profile:debug`, `sourceDirty:true`. It cannot qualify for a production draft. Its initial local build attempt failed to find Cargo in Tauri's child environment; retrying with a process-local normalized PATH succeeded without modifying saved environment variables.

Local validation and GitHub CI remain separate evidence. The preceding `cbd05ff` CI run covers the native acceptance documentation; new changes require their own PR checks. Do not reuse an older green SHA as current-head evidence.

## Desktop Draft Admission

The helper requires canonical stable core/version parity and refuses `--debug` without `--test-build`. Production staging requires a clean Git tree before and after building and the expected immutable source SHA. Explicit test builds may record dirty source, but their evidence cannot pass the production handoff.

The desktop workflow requires release profile, clean exact source, production identity, no native test transport, matching CLI/version/platform and a canonical signing key ID. It recomputes the package SHA-256/size and checks the checksum record. It selects exactly the canonical package, signature, checksum, keyed feed, candidate and evidence; unrelated files cannot satisfy an asset-count check. Missing assets, changed bytes, test/debug/dirty/wrong-source evidence, ambiguous stages and existing releases are refused before creation. Creation always includes `--draft` and the reviewed source target, with no overwrite. The staging helper's signature verifier remains responsible for cryptographic admission.

Guardrail: checking source only at workflow checkout does not bind the eventual package to that source. Record and check source/profile in package evidence as well; test the real workflow handoff, not a duplicate implementation of it.

## Remaining Release Sequence

1. Finish reviewing PR #1 for version 0.4.0 and obtain exact-head green CI after the version/lockfile/notes changes. Retain the legacy client and V2 route throughout the 0.4 series; choose any removal/end-of-support date separately after an accepted installed migration. Merge is a separate authorized action; protected staging accepts only master.
2. Run the protected desktop/server draft workflows from the reviewed master SHA. Required environment approval is the configured owner gate. Check the resulting signatures against independently retained public trust and inspect exact source, version, platform and package evidence. Keep both releases as drafts.
3. Admit the exact production artifacts: verify the Windows production installation/update identity and Linux runtime/persistent-service behavior, with any required manual checks scoped and estimated in advance. Reuse the [completed native session](native-history-acceptance.md) and installed updater evidence; do not repeat unchanged synthetic micro-checks. Interrupted installation, omitted native recovery paths and other desktop platforms remain explicit gaps.
4. Review a concrete live server/SSH/startup transition and return path before separately authorizing it. Preserve the legacy service/client and explain in-memory shelf loss on restart. Publication is another separate action; server/legacy releases must not displace the desktop keyed-feed latest release.

No current service, SSH restriction, user clipboard or installed startup entry is changed by this preparation. The manual history session is closed.

## Update Triggers

- A release gate closes, fails, or changes scope; a candidate version/platform is chosen.
- Exact-head CI, protected signing or production artifact evidence becomes available.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md), and [testing](testing.md).
- [Release operations](releasing.md), [desktop updates](desktop-updates.md), and [server maintenance](server-maintenance.md).
- [Compatibility/cutover](compatibility-and-cutover.md) and [native history acceptance](native-history-acceptance.md).
