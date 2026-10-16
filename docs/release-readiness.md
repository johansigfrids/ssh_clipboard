# Rework Release Readiness

## Purpose

Keep the release candidate's automated evidence, prepared material, and remaining approval/platform gates in one place. Version 0.4.0 is merged with verified signed drafts and an accepted Windows production installation/text round trip. Publication, startup ownership and live server migration remain separate.

## Key Files

- `CHANGELOG.md`: versioned 0.4.0 candidate notes; assign the actual publication date at release time.
- `.github/workflows/desktop-release.yml`, `server-release.yml`: master-only protected draft staging.
- `desktop/scripts/build-installed.mjs`, `release-source.mjs`: desktop source/package evidence.
- `desktop/tests/desktop-release.test.mjs`, `release-source.test.mjs`: source admission and actual draft-handoff refusal tests.
- [Release operations](releasing.md) and [compatibility/cutover](compatibility-and-cutover.md): trust, publication and live migration procedure.

## Candidate Scope

The rework candidate **0.4.0** was implemented on `codex/ssh-clipboard-rework` and merged through PR #1. The owner authorized choosing a sensible version and preparing the PR for merge on 2026-10-16. A minor pre-1.0 increment reflects the substantial desktop/history/update additions while the existing V2 route remains available; it does not claim the broader platform/operational maturity of 1.0. The shared core and both dependency lockfiles carry 0.4.0. The desktop frontend/prototype packages retain their internal 0.0.0 versions; installed staging overrides the app version from the matching core version.

PR #1 merged at `766f6e2659ecf984fdb568da10b8d941d07486d9`; merged-master CI passed all seven jobs. Both protected workflows passed after owner approval, creating verified `desktop-v0.4.0` and `server-v0.4.0` drafts from that exact source. The owner installed the Windows production package and confirmed a native text round trip; the exact Linux artifact passed isolated SSH, released-client and runtime-service checks. See [production artifact acceptance](release-0.4.0-acceptance.md) for immutable hashes, run links and boundaries.

The notes retain a 0.4.0 section explicitly marked Unreleased, with no publication date. Neither draft is published; latest remains v0.3.0. Synthetic updater versions and the pinned published v0.3.0 compatibility fixture remain unchanged; they are evidence inputs, not current application versions.

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
| Protected production builds | Reviewer gates, master-only branch rules and public trust were rechecked; both approved 0.4.0 workflows signed successfully. Independent downloaded-package verification passed without reading private inputs. See the exact-artifact report above. |

The synthetic package was built from the in-progress preparation tree and is explicitly marked `testOnly:true`, `profile:debug`, `sourceDirty:true`. It cannot qualify for a production draft. Its initial local build attempt failed to find Cargo in Tauri's child environment; retrying with a process-local normalized PATH succeeded without modifying saved environment variables.

Local validation and GitHub CI remain separate evidence. PR-head `04446f4` and merged-master `766f6e2` CI both passed all seven jobs. Documentation-only acceptance follow-ups do not change the recorded artifact source. Do not reuse an older green SHA as current-head evidence for new implementation changes.

## Desktop Draft Admission

The helper requires canonical stable core/version parity and refuses `--debug` without `--test-build`. Production staging requires a clean Git tree before and after building and the expected immutable source SHA. Explicit test builds may record dirty source, but their evidence cannot pass the production handoff.

The desktop workflow requires release profile, clean exact source, production identity, no native test transport, matching CLI/version/platform and a canonical signing key ID. It recomputes the package SHA-256/size and checks the checksum record. It selects exactly the canonical package, signature, checksum, keyed feed, candidate and evidence; unrelated files cannot satisfy an asset-count check. Missing assets, changed bytes, test/debug/dirty/wrong-source evidence, ambiguous stages and existing releases are refused before creation. Creation always includes `--draft` and the reviewed source target, with no overwrite. The staging helper's signature verifier remains responsible for cryptographic admission.

Guardrail: checking source only at workflow checkout does not bind the eventual package to that source. Record and check source/profile in package evidence as well; test the real workflow handoff, not a duplicate implementation of it.

## Remaining Release Sequence

The second login also failed after quoting. Windows' registry provider sees no new startup/uninstall registration and still sees the old agent startup command, despite opposite results inside the automation process. Reinstallation from the owner's normal Windows desktop and authoritative registry verification are required before another sign-in check. Prior automation-view registry results do not close installation/startup/legacy-registration cleanup gates; see the [acceptance correction](release-0.4.0-acceptance.md#remaining-gates).

**Blocking desktop defect:** the production login check failed despite an enabled registry entry. Its spaced executable path was unquoted. The installed entry is repaired and the source adapter has a quoting fix; real repaired-login confirmation and a newly versioned signed fixed installer are pending. Do not publish or overwrite the 0.4.0 desktop draft. See [the acceptance report](release-0.4.0-acceptance.md#remaining-gates).

1. Merge, exact-source CI, protected draft signing, independent artifact verification, Windows installation/text paste and Linux isolated runtime admission are complete; retain both releases as drafts.
2. The owner authorized full removal of the legacy Windows agent after the production text check; binaries/startup/PATH/obsolete configuration were removed with the new shared settings preserved. Enable and verify production desktop startup next. Review Linux persistent-service permissions/login/boot plus the concrete live server/socket/SSH transition and return path before separately authorizing them; explain in-memory shelf loss on restart. Retained configuration backups and the published legacy package provide the client return path.
3. Reuse the [completed native session](native-history-acceptance.md) and installed updater evidence. Public production delivery, interrupted installation, omitted native recovery paths and other desktop platforms remain explicit gaps; do not repeat unchanged synthetic micro-checks.
4. Publication is another separate action. Server/legacy releases must not displace the desktop keyed-feed latest release. Retain the legacy client/V2 route throughout the 0.4 series; choose any removal date after accepted migration.

The authorized Windows production installation and text clipboard check are complete. No live Linux service or SSH restriction has changed. The legacy Windows startup entry was removed at the owner's explicit request. The new entry is enabled and its command was repaired after the failed login check; actual repaired-login evidence remains pending. The manual history session is closed.

## Update Triggers

- A release gate closes, fails, or changes scope; a candidate version/platform is chosen.
- Exact-head CI, protected signing or production artifact evidence becomes available.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md), and [testing](testing.md).
- [Release operations](releasing.md), [desktop updates](desktop-updates.md), and [server maintenance](server-maintenance.md).
- [Compatibility/cutover](compatibility-and-cutover.md) and [native history acceptance](native-history-acceptance.md).
