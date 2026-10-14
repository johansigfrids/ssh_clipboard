# Uncertain Share Recovery

## Purpose

Recover a lost shelf upload acknowledgment without recapturing a newer clipboard, repeating a write automatically, or creating another item after the retry record expires. Legacy V2 shares retain conservative uncertainty messages and have no operation-ID recovery.

## Key Files

- `src/shelf.rs`: bounded retry metadata lookup, expiry, and epoch-monotonic clock.
- `src/session_protocol.rs`, `src/shelf_service.rs`: OperationStatus/Operation and deadline-protected RetryShare.
- `src/client/session.rs`, `src/client/connection.rs`, `src/client/session_transport.rs`: correlation, limits, and retry uncertainty.
- `desktop/src-tauri/src/pending.rs`, `desktop/src-tauri/src/panel.rs`: frozen capture and explicit recovery actions.
- `desktop/src/App.svelte`: Check status, Retry captured share, and Discard recovery data.

## Frozen Capture and Retention

Share samples the native clipboard once, before waiting for SSH. After negotiation and a fresh read-only operation clock query, the desktop retains one bounded capture in RAM for the active profile: original bytes, representation, device/name, capture timestamp, server epoch, operation UUID, and original retry deadline. A typed pre-send/rejection failure drops the new pending capture. Cancellation during upload or an unacknowledged mutation retains it. An earlier uncertain share blocks another capture until resolved or discarded, including from a direct shortcut.

Pending payload bytes use the selected encoded-item budget, never an unbounded history of captures. The two-second settings watcher drops expired bytes; operation/device/epoch metadata can remain for a read-only status check. The original deadline is never renewed. Profile removal/switch and Quit clear recovery data. Settings changes within the same profile can retain it, but every network action still checks the current server epoch and negotiated policy. Nothing is written to disk or logs. Frontend IPC receives only the operation ID and whether a retry can be offered.

## Check Before Repeating

OperationStatus accepts the original device and operation IDs and returns correlated metadata with no payload or payload lease. It does not extend retry retention. A retained acknowledgment can confirm acceptance even when the item has since expired, been deleted, or been cleared. Unknown means no retained record; it does not prove that the original upload failed. Epoch changes remain uncertain and prohibit sending the captured bytes to the new epoch.

Check share status never uploads. Retry captured share first performs the same status check. A known acknowledgment confirms the earlier share without resending; a known removed item is not resurrected. Only an unknown outcome inside the original safe window can send the frozen bytes, and only after that explicit user click. It retains the original UUID, device/name, representation, bytes, and capture timestamp. No recovery path reads or changes the OS clipboard. Narrower current limits reject the capture without resizing or replacing it.

Discard recovery data drops only local recovery state. It does not delete a server item or claim the earlier share failed.

## Server-Enforced Deadline

The `operation_status` and `deadline_share` capabilities are negotiated independently. Operation replies include an epoch-monotonic time. Before the first upload, obtain a fresh sample T0 and freeze D = T0 + negotiated retry retention. Do not derive it from an old cached handshake, wall-clock timestamps, a later status check, or a new retry attempt.

Both the original upload and all retries use RetryShare, a wrapper around exactly one ordinary Share plus D. Nested/non-Share wrappers are invalid. The server checks its epoch-monotonic clock immediately before committing, under the shelf mutex after content validation. At or beyond D it returns RetryExpired without creating an item. A local retry guard additionally expires conservatively; the server deadline is authoritative if network/validation time consumes the remaining window.

If the original upload committed at T1, its retry record expires at T1 + retention, which is at least D. No protected request can commit after D, so expiry of that record cannot turn a delayed original or retry into a second item. Ordinary legacy/unprotected SDK Share calls do not gain this guarantee merely because a later caller invents a retry deadline: integrations must protect the original request too. Peers without deadline support can still share normally and report/check uncertainty, but the desktop does not offer a captured retry.

## Evidence and Remaining Acceptance

Stream tests deliberately commit and drop the first client without reading its acknowledgment, reconnect, query the original operation, retry its protected bytes, and verify one item with the same ID. Service probes expire deduplication after a removed item and reject a late protected request without retaining payload memory. Core probes show that lookup neither pins bytes nor renews records and that an unknown operation can coexist with its still-stored item.

Desktop probes preserve capture identity/bytes/time/deadline, reject other epochs and stale operation IDs, expire local bytes, and block another native read while recovery is pending. Rendered tests require explicit Check/Retry/Discard commands and issue no Share/Copy command during recovery. The native Linux proxy probe checks operation metadata on the existing owned connection. Native Windows lost-ack recovery and new-server SSH acceptance remain separate checks.

## Update Triggers

- Retry window, deadline/correlation, operation retention, or profile isolation changes.
- New retry automation, persistence, additional representations, or native acceptance evidence.

## Related Docs

- [Owned sessions and cancellation](persistent-sessions.md)
- [Session protocol](session-protocol.md)
- [Shelf core](shelf-core.md)
- [Desktop history](desktop-history.md)
- [Implementation plan](implementation-plan.md)
