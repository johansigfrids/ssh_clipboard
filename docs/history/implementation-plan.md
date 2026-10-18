# SSH Clipboard Rework Implementation Plan

> Historical record, archived 2026-10-18. Version, latest-release and pending-gate statements below belong to the recorded source/session. Use [the docs index](../index.md) for current guides; preserve exact identities and unobserved limits when adding corrections.

## Purpose

Track the completed rework and the explicit scope decisions that follow it. The core idea remains explicit cross-machine clipboard sharing over SSH, with a discoverable panel, real settings, temporary history and verified updates.

**Status: delivered for the agreed scope; reconciled 2026-10-18.** Windows x64 and Apple Silicon desktop 0.4.2 and Linux x64 server 0.4.0 are published. The owner machines have completed their clean migrations and legacy installation cleanup. The Mac installed update/restart rehearsal and its cleanup also passed. The original implementation phases below are closed for this scope.

The owner has no Linux desktop and has deferred that platform. Untested rare scenarios are recorded as evidence limits, not a queue of mandatory manual tests. The final source/distribution retirement policy is in [legacy retirement](../legacy-retirement.md); it does not change already published binaries or live installations.

## Key Files

- `src/configuration.rs`, `src/configuration/`: shared settings, guarded migration and backups.
- `src/client/`, `src/client_actions.rs`: transport, diagnostics and explicit transfer actions.
- `src/shelf.rs`, `src/shelf_service/`, `src/session_protocol/`: bounded temporary history and session contract.
- `src/shelf_daemon.rs`, `src/session_proxy.rs`, `src/server_management.rs`: Linux endpoint and maintenance.
- `desktop/src/`, `desktop/src-tauri/`: quick panel, settings, retained clipboard worker and signed updater.
- `.github/workflows/desktop-release.yml`, `server-release.yml`: protected signed drafts.
- [Architecture](../../ARCHITECTURE.md), [release readiness](release-readiness.md), [retirement policy](../legacy-retirement.md).

## Supported Delivery

| Component | Delivered state |
| --- | --- |
| Windows x64 desktop | Published 0.4.2. Native panel/settings/text/image/history and installed updater/startup evidence are recorded separately by tested build. |
| Apple Silicon desktop | Published 0.4.2. Actual legacy import, text share/copy, login startup, panel shortcut and clean retirement pass; isolated real update/restart passes. |
| Linux x64 server | Published managed 0.4.0 shelf, accepted user service/SSH routes, signed maintenance and legacy installation retirement. |
| Clipboard/stdio CLI | Shared settings and explicit transfers remain. Default source builds include native clipboard support; retired agent features require explicit opt-in. |

Immutable source/package identity and public delivery are in [0.4.2 publication acceptance](release-0.4.2-acceptance.md). The independent Linux release and live service are in [Linux cutover acceptance](linux-cutover-acceptance.md). Documentation and retirement commits after a release do not replace its signed package bytes.

## Completed Phases

| Original phase | Outcome and evidence |
| --- | --- |
| 1. Desktop foundation | Tauri/Svelte shipped for Windows/Apple Silicon; retained clipboard ownership, tray/menu, launcher, panel shortcut and single-instance behavior. [Desktop evidence](../desktop-development.md), [Mac cutover](macos-cutover.md). |
| 2. Shared services/configuration | Shared desktop/CLI schema, profile identities, overrides, atomic coordinated saves, live reload, corruption preservation and backed-up migration. Server builds exclude desktop dependencies. [Configuration](../shared-configuration.md), [migration](../configuration-migration.md), [build boundaries](../build-variants.md). |
| 3. Quick panel/settings | Visible Share/Copy and connection state; connection profiles/diagnostics, shortcut recording/conflicts, behavior and update controls. Deterministic content labels and private previews; no automatic sharing or pasting. [Shortcuts](../desktop-shortcuts.md), [notifications](../desktop-notifications.md). |
| 4. Shelf/protocol | Temporary bounded text/PNG history, item/epoch/order identities, negotiation, expiry/eviction, atomic shares, deduplication, selected fetch, delete/clear and optional V2 adapter. [Shelf](../shelf-core.md), [session contract](../session-protocol.md), [history](../desktop-history.md). |
| 5. Persistent sessions/recovery | Owned foreground/control SSH connections, metadata Watch, bounded cancellation/reconnect, stale-event guards and uncertain-share lookup/frozen retry/discard. [Sessions](../persistent-sessions.md), [recovery](../share-recovery.md). |
| 6. Desktop packaging/updates | Stable installs, actual-state startup controls, protected signing, independent package identity/byte verification, bounded explicit updater and key recovery/rotation policy. Windows and Mac installed paths have distinct evidence. [Updates](../desktop-updates.md), [Mac updater](macos-updater-acceptance.md), [publication](release-0.4.2-acceptance.md). |
| 7. Server maintenance | Immutable user-owned versions, stable launcher/unit, pinned signing trust, bounded health, restart-failure rollback and recovery. Disposable real-service and exact live-package checks pass. [Maintenance](../server-maintenance.md), [live cutover](linux-cutover-acceptance.md). |
| 8. Migration/cutover | Preserved actual settings/startup choices, one-time import review, accepted Windows/Mac/Linux owner cutovers, old active installation removal and separate recovery backups. [Compatibility](../compatibility-and-cutover.md), [Mac migration](macos-cutover.md), [retirement](../legacy-retirement.md). |

The earlier phase proposals and provisional statuses remain in Git history. They are not additional unfinished tasks. Feature documents and acceptance reports retain the original contracts and precise native/automated boundaries.

## Contracts That Remain

- Sharing and copying are explicit. Opening, previewing, metadata refresh and reconnect do not sample/share or overwrite the local clipboard; the app does not auto-paste into another application.
- Labels are bounded deterministic previews from actual content, not invented document names or externally fetched titles.
- History is temporary in-memory storage. Effective item, decoded-image, aggregate and transient budgets are bounded and negotiated; legacy endpoints retain their narrower limits.
- System OpenSSH, existing aliases/keys, strict host verification, private sockets and user isolation remain. A transfer-only key does not become a management key.
- Updates authenticate exact package bytes and security-relevant release identity, reject tampering/downgrade and require explicit installation confirmation. Private signing inputs stay in protected release environments and owner password-manager recovery.
- Existing shared settings win over legacy discovery. Corrupt files, original import backups and obsolete-profile isolation remain protected.

## Legacy Retirement: Decision Made

The old tray agent is retired from the default source build and new distribution. The old automatic `v*` release publisher is removed. The supported desktop is the signed 0.4 app; frozen v0.3.0 archives remain historical downloads.

Keep the explicit `agent` compatibility feature, legacy TOML importer, V2 CLI/profile routes and optional server adapter through 0.4. They still support real command/configuration contracts. Their later source/protocol removal is not a scheduled follow-up and requires a separate explicit breaking-change decision. No live compatibility socket, SSH restriction or private recovery backup changes in this cleanup. See [the exact retirement boundary](../legacy-retirement.md).

## Deferred and Non-Blocking

| Item | Decision |
| --- | --- |
| Linux desktop/AppImage, native X11/Wayland and shortcut portal work | Deferred; the owner has no Linux desktop. Existing compilation checks may remain as regression protection. |
| Native Mac image paste and browser-download Gatekeeper approval | Unverified, recorded limits. No further manual campaign is scheduled. Free updater signing does not provide Apple notarization. |
| Interrupted native replacement, Mac native key rotation and rare recovery UI paths | Existing automated/process evidence is retained; omitted native cases are not release blockers or claimed passes. Revisit only for a relevant change or real issue. |
| Actual live-server reboot/last-session logout | Not observed in the cutover session. Linger/unit configuration and service/SSH health are recorded; no disruptive verification is scheduled. |
| Intel Mac and additional package/OS targets | No new target is committed by this delivery. |
| File transfer, disk history, full mesh, OCR/AI naming, HTML/RTF and optional automation | Outside the initial rework. Require a new product decision rather than silently extending this plan. |

## Future Work and Verification Budget

There is no remaining mandatory implementation or manual-verification phase for the agreed Windows/Apple Silicon/Linux-server delivery. Ordinary bugs, dependency maintenance and future feature requests become separately scoped work.

Use existing automated, process and artifact checks first. Request a native check only for a concrete user-visible risk that those checks cannot establish; state the expected result and a short finite checklist before starting, and guide actions one at a time. Do not reopen the completed migration/history/updater campaigns or infer a reboot/deployment from an evidence gap. Keep untested scenarios labeled untested.

## Update Triggers

- A real defect or approved feature changes a shipped contract.
- The owner requests a deferred platform or explicitly ends a compatibility contract.
- New evidence contradicts an accepted result or changes a recorded limit.

## Related Docs

- [Architecture](../../ARCHITECTURE.md), [docs index](../index.md), [writing conventions](../writing-docs.md).
- [Release readiness](release-readiness.md), [release operations](../releasing.md), [testing](../testing.md).
- [Legacy retirement](../legacy-retirement.md), [compatibility/cutover](../compatibility-and-cutover.md).
- [Windows native history](native-history-acceptance.md), [Mac updater](macos-updater-acceptance.md), [Linux cutover](linux-cutover-acceptance.md).
