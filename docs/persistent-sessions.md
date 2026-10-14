# Owned Persistent SSH Sessions

## Purpose

Document the Phase 5 SSH owners, explicit operation cancellation, and metadata-only history recovery. Upload reconciliation and protected captured retries are documented in [share recovery](share-recovery.md).

## Key Files

- `src/client/connection.rs`, `src/client/connection/tests.rs`: child/session ownership and lifecycle probes.
- `src/client/session.rs`: negotiated sequential framing and reply validation.
- `src/client/session_transport.rs`: one-invocation CLI adapter and shared safe errors.
- `src/client/monitor.rs`: bounded metadata watch, recovery/backoff, and typed stop conditions.
- `desktop/src-tauri/src/panel.rs`, `desktop/src-tauri/src/history.rs`: active-profile cache and operations.
- `desktop/src-tauri/src/operation.rs`: lock-independent cancellation, identities, and finishing boundaries.
- `tests/native_shelf_server.rs`: real disposable process/socket evidence.

## Ownership and Reuse

`Connection::connect` owns the child and pipes and negotiates before use. A lifetime reader drains stderr in fixed 4 KiB chunks into bounded 8 KiB head/tail capture. Public errors contain classifications, not private remote text. Launch diagnostics record only error kind and numeric OS code, without paths, targets, or payloads.

The desktop caches one connection for the active shelf profile. List/pages, preview selection, copy checks, share, delete, and clear reuse it. Saved-connection diagnostics remain a separate explicit check. Profile/endpoint/transport changes discard the cache and history; preference-only changes can retain them. Full SSH configuration and session policy must match before reuse. A child known to have exited is closed before an explicitly requested action opens another session.

The selected shelf profile also owns one dedicated metadata connection, for a bound of two cached SSH children/session slots per active profile. The sequential wire cannot wait in Watch and serve an immediate user transfer simultaneously. Keeping the bounded control owner separate prevents background work from taking the transfer mutex across network I/O. Legacy profiles retain their explicit one-shot behavior and start no monitor. A separately requested connection diagnostic remains a temporary additional check.

Operations take the connection out of the cache before awaiting and return it only while usable. Dropping that owned operation scope drops its child/reader instead of caching partial framing. The stream driver also poisons itself before an exchange.

Share freezes exactly one capture before waiting for SSH. A later clipboard change must not replace it after a slow connection. Local policy bounds capture; negotiated limits then check the frozen bytes and native RGBA8 PNG descriptor before upload. A narrower limit/failure never triggers another sample, resizing, or replay. Preparation dimensions stay out of preview IPC.

Native clipboard admission belongs to the owner-thread job. An atomic permit rejects additional reads/writes until that job finishes, even if its async caller was dropped. A bounded channel alone allows a later write to queue behind a cancelled read; never treat dropping the receiver as stopping OS work. Release admission before delivering the result so the next explicit action can proceed immediately. Already running native work remains synchronous and cannot be forcibly interrupted.

## Failure and Cleanup

Typed rejections can retain a healthy stream. Partial/malformed replies, timeout, process loss, or epoch change invalidate it. Failed connections close pipes, kill/reap the direct child under a separate two-second bound, and finish/abort diagnostics. Explicit Close allows bounded shutdown/normal exit and kills a stuck child. Synchronous invalidation/drop kills and aborts immediately, with Tokio's best-effort drop reaping.

The diagnostic reader has its own abort-on-drop owner. Cancellation while cleanup temporarily holds its handle must still abort it: dropping a raw JoinHandle would detach the task. An unacknowledged mutation retains its original epoch and operation UUID where applicable in a typed uncertainty result after session cleanup. Validated acknowledgments remain authoritative. No action is automatically repeated or sent into a replacement epoch. Selected fetch/mutation checks compare loaded and negotiated epochs before sending anything.

Uncertain shelf uploads now retain one frozen capture and support read-only operation reconciliation and explicit deadline-protected retry/discard; see [share recovery](share-recovery.md). Native acceptance remains open.

## Background History and Reconnection

The monitor starts with a complete, sequence-anchored metadata snapshot, then Watch from that snapshot sequence. Changes and gaps trigger another complete snapshot; equal-sequence heartbeats retain the list. Watch subscription starts before the service checks its state, closing the snapshot/watch race. A peer without Watch uses bounded Health polling instead. No path in this driver fetches payloads, samples the local clipboard, uploads, deletes, clears, or writes a clipboard.

Both input and output use single-value watch channels. Slow consumers retain one latest update and a bounded snapshot, without accumulating event/payload queues. Waits are at most five seconds and one third of the selected operation deadline. An immediate heartbeat is throttled to at least 250 ms; a full metadata snapshot also refreshes every 30 seconds. Inconsistent pagination is retried after a bounded delay. Expiry and legacy uploads notify the same server sequence channel.

Recoverable network/DNS/timeout/EOF/daemon-busy failures reconnect after 1, 2, 4, 8, 16, then at most 30 seconds. Authentication, host trust, local configuration, missing proxy, unsupported capabilities, and protocol/content failures pause for action. Settings/revision changes cancel the old owner, including a blocked handshake/watch. Reconnect history explicitly restarts it without any transfer. Validated snapshots reset backoff. This is metadata recovery, not automatic replay of a pending Share or Copy.

The desktop applies updates only to the current profile, full settings revision, and monitor generation. Snapshots carry their request-start instant; a completed user operation establishes a barrier so an older background response cannot replace its result, even across server epochs. Within an epoch, older sequences are rejected. Metadata updates retain an explicitly selected immutable preview only while that item survives; a new epoch or removed selection clears it. Background metadata never fills a preview from payload bytes. Hidden-preview projection still removes content labels before frontend IPC.

Quit aborts and joins both monitor tasks and closes the cached data owner under a two-second shutdown bound before stopping the native clipboard owner. Do not rely on process exit or dropping a raw JoinHandle to clean up background SSH children.

## Explicit Cancellation

The desktop publishes the active operation through a separate controller. `get_operation` and `cancel_operation` never acquire the transfer/session mutex. Decimal-string IDs target one operation; monotonically increasing decimal-string generations reject stale frontend events, including values beyond JavaScript's safe integer range. Startup subscribes before reading the operation snapshot, so Cancel can appear while the panel snapshot waits behind a background transfer. Only static operation labels cross this channel.

Connection tests, refreshes, previews, shares, copies, and history mutations expose Cancel while waiting. Cancelling drops the awaited operation future, closes an owned in-flight shelf connection, clears cached content/connection evidence, and does not queue a replacement. Native clipboard work already running finishes on its owner thread under the admission guard above. No cancellation path posts a clipboard write or retries a server action.

Copy atomically enters a non-cancellable finishing stage before posting its native write. Cancel wins first and prevents that write, or the write wins first and Cancel reports that it is already finishing. A validated shelf mutation acknowledgment similarly enters finishing before any later metadata refresh; a concurrent late Cancel must not turn a confirmed mutation into apparent cancellation. An unacknowledged mutation reports that the server may already have applied it. Cancellation is not rollback. Shelf shares retain one bounded capture for an explicit acknowledgment check or protected retry; delete/clear and legacy transfers retain conservative refresh guidance.

The Open panel shortcut uses the actual registered Open ID outside the transfer mutex, so it can reveal Cancel during a background operation. Direct Share/Copy still reserve the session immediately and never queue. Registration changes and rollback refresh this Open ID from actual inventory, not a settings draft.

## Evidence and Fixture Guardrails

Portable subprocess probes cover several requests on one child, flooded-but-bounded stderr, uncertainty/epoch retention and reaping after a lost acknowledgment, and cancellation of a reader taken for cleanup. A desktop fake-clipboard probe checks one capture precedes SSH failure and no clipboard write occurs.

A gated native clipboard probe drops the async reader while the owner remains blocked, rejects later reads/writes without queueing, then releases the owner and verifies only a newly requested write executes. This verifies cancellation admission without touching the OS clipboard.

Controller probes cover waking an already waiting operation, dropping pending work, rejecting stale Cancel IDs, lost-acknowledgment uncertainty, cancellation before native write admission, and acknowledgment/write finishing boundaries. Rendered tests cover Cancel during a pending transfer, startup during a background operation, stale event rejection above JavaScript's safe integer range, and disabled Cancel while finishing. Native desktop Cancel, panel opening during a transfer, and remote cleanup still need a combined Windows acceptance pass.

A native WSL probe shares/lists/fetches/checks health through one real proxy launch, kills/restarts the disposable daemon, verifies no hidden replacement/replay, then explicitly opens a new empty epoch. This is local process/socket evidence, not remote SSH/native paste acceptance.

Metadata recovery probes verify changed snapshots, cancellation/drop of a blocked old profile, paused authentication, explicit retry, bounded delays, and recovery after a synthetic network failure. The native process probe additionally exercises Watch change/heartbeat replies over the real proxy. Desktop tests preserve selected previews, reject older sequences/request-starts/revisions, and render background updates/reconnect without issuing transfer commands. These are automated/process checks; new-server SSH and native Windows recovery acceptance remain separate.

Parallel native tests exposed Linux ETXTBSY on a freshly written shim: a fork can retain its writable descriptor before CLOEXEC cleanup even after the writing parent closes it. A disposable fork/write/exec probe reproduced errno 26 while the child retained the writer and successful exec after it exited. Native process cases now run serially; concurrent clients within each case remain concurrent. Fixture Tokio runtimes use two workers rather than multiplying the host's 32 workers per subprocess. Do not add upload retries to hide a fixture race.

## Update Triggers

- Cancellation, background health/backoff, pending uploads, subscriptions, or reconnect behavior changes.
- Ownership/cache/uncertainty semantics or native/SSH acceptance evidence changes.

## Related Docs

- [Desktop history](desktop-history.md)
- [Shelf client](shelf-client.md)
- [Session protocol](session-protocol.md)
- [Native endpoint](shelf-server.md)
- [Implementation plan](implementation-plan.md)
