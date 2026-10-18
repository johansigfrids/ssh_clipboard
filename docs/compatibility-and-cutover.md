# Compatibility and Cutover

## Purpose

Record the rework's protocol/configuration boundaries and the sequence for an owner-authorized installation cutover. The [live Linux report](linux-cutover-acceptance.md) records completed managed installation/SSH acceptance and legacy retirement; checklist steps alone are not acceptance evidence.

## Key Files

- `src/protocol.rs`, `src/session_protocol.rs`: separate V2 and SCBS bootstrap/session formats.
- `src/shelf_daemon.rs`, `src/shelf_service.rs`: one shelf with an optional explicit V2 adapter.
- `src/server_management.rs`, `src/server_releases.rs`: pinned service installation and recoverable executable selection.
- `src/configuration.rs`, `src/configuration/migration.rs`, `desktop/src-tauri/`: configuration migration and desktop runtime.
- [Server maintenance](server-maintenance.md), [configuration migration](configuration-migration.md), and [signed release operations](releasing.md).

## Connection Matrix

The legacy wire version is 2 (`SCB1`). The new `session-proxy` negotiates SCBS bootstrap/session version 1. They are deliberately separate endpoints; no payload sniffing upgrades a legacy connection. Unsupported session versions are rejected during Hello. CLI `push/pull/peek` and a desktop profile's legacy mode use V2; CLI `shelf` and shelf-mode profiles use the new session.

| Client route | Legacy `daemon` + `proxy` | Shelf with only `session-proxy` | Shelf with explicit V2 compatibility socket |
| --- | --- | --- | --- |
| Legacy agent/CLI or rework panel in legacy mode | Latest text/PNG via V2 | No shared V2 endpoint | Latest text/PNG via `proxy` on the compatibility socket |
| Rework panel in shelf mode | Unsupported endpoint; use legacy mode | History, explicit share/copy/delete/clear and metadata Watch | Same shelf features; legacy clients see the same latest item |
| Rework `shelf` CLI | Unsupported endpoint | Metadata and explicit file/stdin transfers; no OS clipboard access | Same item-aware operations and shared latest item |

The compatibility adapter must be explicitly configured when the shelf starts. Running separate legacy and shelf daemons without it creates separate stores. Occupied sockets are preserved and startup fails instead of displacing a running service. Legacy clients have no history/delete/deduplication controls. Their default frame cap remains 10 MiB; a larger latest shelf item can be visible in metadata yet refused by a legacy Get. Increasing a client preference does not increase another endpoint's policy. New clients negotiate encoded/decoded limits and must respect the narrowest policy.

Current-source native process tests prove shared V2/session storage and refusal of occupied endpoints. Retained V2 fixtures prove the framing contract. The actual published Linux x86_64 v0.3.0 CLI/proxy/daemon now also passes private old/new process-route checks: exact Unicode/whitespace and PNG bytes, shared history/latest-item changes, size refusal with metadata access, and fixed legacy endpoint refusal without mutation. See [released-binary evidence](released-compatibility.md). This establishes that protocol pairing, not every older release/platform or an installed migration. The owner subsequently approved [legacy retirement](legacy-retirement.md): default builds and distribution retire the old tray agent, while explicit source compatibility and V2/import contracts remain through 0.4.

## 0.4 Compatibility Commitment

For the 0.4 series, retain V2 CLI/profile/server routes, migration readers and the explicit opt-in agent source compatibility path. The old tray agent is retired from default builds and new distribution as of 2026-10-18. The tested released pairing is Linux x86_64 v0.3.0 with the rework routes; this is not a promise about every historical platform/version. Any removal or end-of-support date requires a later explicit decision after installed migration acceptance. The core version bump does not change either wire version or automatically move a profile/service to the shelf.

## SSH Route and Privilege Boundary

An existing `command="... proxy"` key remains a V2 key, regardless of the requested SSH command. It cannot reach `session-proxy` or maintenance commands. A shelf-only key needs an explicitly reviewed absolute `session-proxy` forced command, with the correct socket path if customized. Use distinct aliases/keys for separate fixed endpoints during migration; do not introduce a general shell dispatcher to make a transfer key a management key. Preserve host verification and no-PTY/forwarding restrictions.

Managed local health proves the selected version answers at its saved socket. Separately test noninteractive SSH command resolution: `~/.local/bin/ssh_clipboard` must actually be selected by PATH, or the forced command must name it explicitly. The old `/usr/local/bin` link may remain on another route; do not rewrite a shared global link automatically. Custom legacy/shelf socket arguments must match the daemon configuration. Management uses a separately authorized unrestricted connection or a local server command.

## Configuration and Startup

Legacy agent TOML imports only when the new shared file is missing, with an exact backup. Existing shared settings win. Schema 1 prototype settings receive an exact backup before their first schema 2 save. Connection options, transfer limits and legacy shortcuts/startup preferences are retained; direct shortcuts stay disabled until explicitly enabled. Imported startup preferences do not register the new app automatically. Corrupt/future/ambiguous documents are preserved and reported.

The installed rework and legacy agent can otherwise both own startup/shortcuts. During cutover, explicitly quit the old agent, review its startup registration and enable the new installed registration only after the new app works. Keep the old executable/configuration for a return path. Restoring an old configuration backup restores that snapshot, not subsequent edits. See [migration recovery](configuration-migration.md).

Imported settings now have a one-time review in the panel and Settings showing the saved connection/device, panel/direct shortcuts, retained startup choice and recovery backup. Explicit dismissal saves only a guarded receipt; it never stops the old agent or changes native registration. Automated persistence/race/rendered checks pass; the [2026-10-16 Windows session](native-history-acceptance.md) confirms synthetic import review and saved dismissal/profile after relaunch. Actual user-file import and installed cutover remain separate from the installed updater tests.

## Server-First Sequence

1. Record target UID, actual service/unit and executable, socket paths/limits, client SSH routes and forced commands. Preserve the old executable/unit/configuration and a return command. Establish independent signing trust and a verified new package first.
2. On a disposable user service, prove install survives deleting the download, real enable/restart/login behavior, failed-update rollback and journal recovery. Confirm runtime compatibility of the exact package. Fake-controller/source tests do not waive this step.
3. Review the live transition: which service stops, which sockets the shelf will own, and the client/forced-command route. Warn that the in-memory clipboard will be cleared. Separately authorize that concrete change before touching the existing service.
4. Install the managed shelf with reviewed saved limits and, if required, the explicit V2 compatibility socket. Verify local selected-version protocol health, then each actual SSH route. Existing V2 clients should share/copy the same latest small item as a shelf client.
5. Move one desktop profile to shelf mode. Check history selection, explicit text/image share/copy, delete/clear, metadata refresh from another machine, reconnect and cancellation. Retain legacy mode as the reduced-feature client return path.

## Desktop-First Sequence

1. Verify the signed installer independently and preserve the current configuration/agent startup details. Install the new app, keep its profile in legacy mode and confirm actual paste into another application.
2. Explicitly handle old-agent startup/shortcut ownership, then test the new installed start-at-login choice across login. Do not remove the retained old client yet.
3. Perform the separately reviewed server transition above; switch the chosen profile to shelf only after that endpoint and SSH route pass. Settings changes do not deploy a server or widen a forced-command key.

## Owner-Requested Clean Server Replacement

The owner requested a clean Linux 0.4 upgrade on 2026-10-16, including removal of the old installation from active use. After admitting the exact new package and live routes, remove the disabled legacy unit/enablement links, obsolete installed binaries/download archive and obsolete global command link, after verifying each belongs to this installation. Preserve an exact, private rollback copy separately from active installation paths. Do not treat a source checkout, unrelated configuration or a shared global command as disposable without inspecting ownership, contents and references first.

The new shelf's explicit V2 adapter provides the retained 0.4 compatibility commitment; keeping the protocol does not require retaining the old daemon or executable on an active route. Verify the user-scoped SSH route before removing an obsolete global link, and check fixed absolute forced-command references without exposing key material. Never widen key restrictions to make cleanup succeed. Root-owned link cleanup and linger may require a human sudo command; do not collect passwords. Finish health/routing acceptance before retiring the old files, and document when post-cleanup rollback requires restoring a privileged link.

The owner-authorized live transition is now complete: signed persistent 0.4.0 installation, fresh session/V2 SSH routes, and removal of the old active installation pass. The owner also confirmed selecting and saving the installed Windows history endpoint. See [live evidence and return-path limits](linux-cutover-acceptance.md). Server 0.4.0 publication/public delivery also pass; actual reboot/last-session logout remain separate.

## Accepted Delivery and Recorded Limits

The published Windows/Apple Silicon desktop and Linux server, owner clean migrations and old active installation retirement are complete. [Desktop publication](release-0.4.2-acceptance.md), [Mac installed updater](macos-updater-acceptance.md) and [live Linux acceptance](linux-cutover-acceptance.md) distinguish exact artifacts, native observations and public delivery.

The owner deferred Linux desktop work and broad further manual checks on 2026-10-18. Native Mac image/Gatekeeper, interrupted replacement, omitted native recovery paths and actual server reboot/last-session logout remain evidence limits, not a blocking checklist. Existing automated and compatibility tests remain useful; revisit only for a concrete change or observed defect. No new manual migration/sign-out session is scheduled.

The old agent is no longer the default build or an automatically published product. Retaining V2, import readers and explicit opt-in agent source through 0.4 preserves the existing compatibility commitment; it does not keep an old agent running on the owner machines. See [retirement policy](legacy-retirement.md) and the [closed plan](implementation-plan.md).

## Update Triggers

- Protocol negotiation, compatibility adapter or forced-command routing changes.
- Configuration/startup migration, supported package/runtime platforms or release gates change.
- Actual installed/native/released-client evidence establishes a support window or cutover result.

## Related Docs

- [Architecture](../ARCHITECTURE.md), [implementation plan](implementation-plan.md), and [testing](testing.md)
- [Shelf server](shelf-server.md), [desktop history](desktop-history.md), [security](security.md), and [docs index](index.md)
