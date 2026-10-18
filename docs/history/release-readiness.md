# Rework Release Readiness

> Historical record, archived 2026-10-18. Version, latest-release and pending-gate statements below belong to the recorded source/session. Use [the docs index](../index.md) for current guides; preserve exact identities and unobserved limits when adding corrections.

## Purpose

Keep delivered release evidence and its explicit limits in one place. The [implementation plan](implementation-plan.md) closes the agreed scope; omitted rare checks and Linux desktop work are deferred and non-blocking. Desktop 0.4.2 is published for Windows x64 and Apple Silicon, with independent package admission and anonymous combined-feed/public-delivery verification. Windows 0.4.1 installed upgrade/startup acceptance remains historical evidence; clean Mac 0.4.2 migration and the isolated Mac update/restart rehearsal pass. The independently signed server 0.4.0 is installed and published with accepted live SSH routes, clean legacy retirement and anonymous public delivery. Actual reboot/logout evidence remains separate.

## Key Files

- `CHANGELOG.md`: published desktop 0.4.2/0.4.1 and server/rework 0.4.0 notes.
- `.github/workflows/desktop-release.yml`, `server-release.yml`: master-only protected draft staging.
- `desktop/scripts/build-installed.mjs`, `release-source.mjs`: desktop source/package evidence.
- `desktop/tests/desktop-release.test.mjs`, `release-source.test.mjs`: source admission and actual draft-handoff refusal tests.
- [Release operations](../releasing.md) and [compatibility/cutover](../compatibility-and-cutover.md): trust, publication and live migration procedure.

## Candidate Scope

The rework candidate **0.4.0** was implemented on `codex/ssh-clipboard-rework` and merged through PR #1. The owner authorized choosing a sensible version and preparing the PR for merge on 2026-10-16. A minor pre-1.0 increment reflects the substantial desktop/history/update additions while the existing V2 route remains available; it does not claim the broader platform/operational maturity of 1.0. The subsequent desktop correction updates shared core and both dependency lockfiles to **0.4.1**. The desktop frontend/prototype packages retain their internal 0.0.0 versions; installed staging overrides the app version from the matching core version.

PR #1 merged at `766f6e2659ecf984fdb568da10b8d941d07486d9`; merged-master CI passed all seven jobs. Both protected workflows passed after owner approval, creating verified `desktop-v0.4.0` and `server-v0.4.0` drafts from that exact source. The owner installed the Windows production package and confirmed a native text round trip; the exact Linux artifact passed isolated SSH, released-client and runtime-service checks. See [production artifact acceptance](release-0.4.0-acceptance.md) for immutable hashes, run links and boundaries.

Desktop `desktop-v0.4.1` was published as latest on 2026-10-16 after separate owner approval; anonymous installer/feed/signature/checksum delivery verification passes. It supersedes the held 0.4.0 desktop draft. The independently verified server-v0.4.0 is published after separate approval and anonymous public-asset verification; the desktop remains latest. The actual legacy archives remain at v0.3.0. Synthetic updater versions and the pinned v0.3.0 compatibility fixture remain evidence inputs, not current application versions.

The initial protected workflows staged Windows x64 desktop and Linux x64 server packages. The expanded 0.4.2 desktop workflow now stages Windows x64 and Apple Silicon together; the signed Mac package has passed the owner clean-migration/text/login/shortcut checks described below. The legacy CLI/agent pipeline remains separate. Linux desktop installation and native Mac image/Gatekeeper behavior remain unverified.

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

The Mac preparation expands that handoff to require Windows x64 and Apple Silicon together. `desktop/scripts/desktop-draft.mjs` admits each exact package, its candidate/feed/signature association, source public-key identity and the Mac archive inspection receipt, then combines the feed without dropping Windows. Candidate/build-evidence filenames carry platform suffixes to prevent collisions; historical published assets are unchanged. Actual handoff-function tests cover both platforms, missing/duplicate stages and feed disagreement. The expanded production workflow passed all five jobs for exact source `b465c4407805f4feb8d4f58cb9d28e209e26ed7e`. Independent downloaded-package verification admits both platform signatures, source/identity/checksums and the combined feed. The owner-requested clean Mac replacement also passes migrated settings, native text share/copy, old startup/config retirement, actual login with one icon/no panel flash, fresh-session SSH health and the panel shortcut. The [Mac cutover report](macos-cutover.md) retains exact package identity and the remaining native Mac image/Gatekeeper gaps. The later [isolated Mac updater rehearsal](macos-updater-acceptance.md) also passes actual replacement/restart, retained test settings and complete cleanup with production files unchanged. Version 0.4.2 was published after that separate approval; anonymous downloads verify all twelve immutable assets and the latest combined feed. See [publication acceptance](release-0.4.2-acceptance.md).

Guardrail: checking source only at workflow checkout does not bind the eventual package to that source. Record and check source/profile in package evidence as well; test the real workflow handoff, not a duplicate implementation of it.

## Delivery Closed and Follow-Up Scope

Desktop 0.4.2 and server 0.4.0 are published with independent exact-source/signature and anonymous public-delivery evidence. Windows/Mac/Linux owner cutovers and active legacy installation cleanup are complete; the installed Mac update/restart rehearsal and test cleanup pass. [Publication acceptance](release-0.4.2-acceptance.md) retains source/package identities.

On 2026-10-18 the owner deferred Linux desktop work and additional broad manual verification of rare scenarios. Recorded native image/Gatekeeper, interruption/recovery and server reboot/logout gaps remain accurately unverified, but are non-blocking and not scheduled test assignments. Automated regression coverage remains. Native checks are reconsidered only for a specific relevant change or real defect, using a short finite checklist.

[Legacy retirement](../legacy-retirement.md) removes the old agent from default builds and retires its automatic publisher while retaining opt-in agent source, V2 routes/imports and backups through 0.4. No live service, installed app or signed asset changes in that cleanup. The [implementation plan](implementation-plan.md) is closed for the agreed Windows/Apple Silicon/Linux-server scope; future platforms/features or compatibility removal need a new explicit scope decision.

## Update Triggers

- A release gate closes, fails, or changes scope; a candidate version/platform is chosen.
- Exact-head CI, protected signing or production artifact evidence becomes available.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../../ARCHITECTURE.md), and [testing](../testing.md).
- [Release operations](../releasing.md), [desktop updates](../desktop-updates.md), and [server maintenance](../server-maintenance.md).
- [Compatibility/cutover](../compatibility-and-cutover.md) and [native history acceptance](native-history-acceptance.md).
