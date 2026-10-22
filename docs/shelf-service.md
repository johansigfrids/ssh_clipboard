# Shelf Session Service

## Purpose

Describe the transport-independent service connecting strict session messages to one shelf. The [Linux listener/proxy](shelf-server.md), [item-aware client](shelf-client.md) and desktop integrate this service; stream tests establish its own lifecycle and resource contracts.

## Key Files

- `src/shelf_service.rs`: connection admission, deadlines, handlers, and maintenance hook.
- `src/shelf_service/tests.rs`: duplex stream lifecycle and action tests.
- `src/session_protocol.rs`, `src/session_protocol/framing.rs`: typed metadata and bounded raw frames.
- `src/shelf.rs`: payload validation/ownership and shelf state.

## Lifecycle and Operations

The service accepts one stream, negotiates Hello/Welcome, then handles sequential share/list/fetch/delete/clear/health, metadata Watch and operation-status requests, including deadline-protected retries until EOF or failure. It checks epoch on every action and echoes request IDs in replies. Concurrent connections share the same shelf and retry records. A new service instance creates a new epoch; no service restart/deployment occurs merely by building the library.

Admission is nonwaiting: default 16 active streams, configurable from 1 to 64. No frame allocation precedes the service semaphore. The native accept loop must additionally cap accepted/task-queued sockets before spawning handlers. Defaults are 5 seconds for the entire handshake, 5 minutes of ready-session header inactivity, and 30 seconds from a ready header through metadata/body/validation/action/reply. The constructor bounds configurable durations. A deadline closes the stream and releases its connection slot and payload leases.

Metadata is parsed and validated before payload admission. If a bounded upload cannot reserve resident memory, drain only its already-validated length through an 8 KiB buffer under the same operation deadline, reply with a typed error, and keep framing aligned. This lets sequential send-then-read clients receive Busy without requiring an unbounded discard buffer or deadlocking on early response backpressure. Malformed/oversized frames close the stream; do not attempt to recover their bodies.

Validate captured uploads in blocking jobs outside the shelf mutex. Blocking jobs never commit state. If their waiter times out/disconnects, the result is dropped and cannot later commit. PNG metadata claims must agree with decoded content. The final synchronous commit obtains a fresh server time under the mutex and rechecks epoch. Unacknowledged commits still require client operation-ID reconciliation; a deadline is not proof that no commit occurred.

Lists are metadata-only and page at 1-16 items. Later pages require the first page's sequence; a concurrent mutation returns ResyncRequired. Fetch returns an immutable lease, then checks encoded and validated decoded size against the receiving peer's limits. Delete/clear do not revoke already fetched bytes; they remove stored entries while pinned leases remain charged until the response finishes or times out. Native decoder size is recorded as well as RGBA dimensions, so 16-bit PNGs cannot bypass a smaller peer decode budget.

The shelf mutex is never held over stream I/O or image validation. Reply metadata is bounded and payload bytes are written directly from a lease. A blocked reader therefore cannot prevent other connections or maintenance from changing the shelf. Public errors use stable codes; raw decoder, payload, and remote diagnostic strings are not put in wire errors.

## Expiry and Adapter Responsibilities

`maintenance()` expires items/retry records and returns epoch/sequence. The native shelf daemon calls it every second even with no traffic. Authentication is outside this generic stream layer: a native listener must enforce private socket/directory permissions, same-UID peers, exclusive daemon ownership, bounded accepted tasks, and safe stale-socket recovery before calling `serve`.

The service now supports bounded metadata Watch requests. A watch channel retains only the latest sequence, notifications include session mutations and maintenance expiry, and no shelf mutex is held while waiting. Slow consumers resnapshot rather than accumulating events. Each observer occupies one normal bounded connection slot. No cancellation request, reconnection driver, or OS clipboard access exists in this service layer. The [native endpoint](shelf-server.md) supplies the listener and optional V2 socket; [persistent sessions](persistent-sessions.md) own client reconnection and cancellation. Installed/native evidence is recorded separately in [Linux cutover acceptance](history/linux-cutover-acceptance.md) and [Windows history acceptance](history/native-history-acceptance.md). The standalone legacy daemon remains single-item; its behavior must not be confused with the shelf adapter.


## Evidence

Duplex tests cover multiple actions in one session, exact Unicode/whitespace bytes, simultaneous identical commits, sequence-safe pagination, epoch/invalid-content rejection, admission and handshake cleanup, incomplete-upload deadlines, unsupported version errors, validated PNG descriptors/native decode budgets, Busy drain/retry, and blocked-reader expiry/lease cleanup. Mixed-client tests cover latest-item visibility, unknown origin, timestamps, clear, size refusal with metadata access, shared admission, retained frame charging, malformed/trailing data, and incomplete-upload cleanup. The blocked-reader probe leaves a response body unread, expires the stored item concurrently, then verifies the retained bytes are released at the write deadline.

These tests prove generic stream/service behavior. They do not prove native socket ownership, process/stdio lifecycle, SSH forwarding, platform packaging, or the desktop's native clipboard behavior.

## Prevention Guidance

- Count removed-but-pinned bytes until the final fetch lease drops; list eviction is not memory release.
- Keep validation jobs free of state mutations, so timed-out detached work cannot upload later.
- Do not write an early Busy reply and wait forever while a sequential client is still blocked writing its body. Bound/drain the validated rejected body or close the stream.
- Use actual decoder output size as well as RGBA dimensions when enforcing a peer's image budget.
- Capture server time under the shelf mutex, never before waiting for validation/another request.

## Update Triggers

- Listener/proxy/compatibility wiring, lifecycle limits, subscriptions, or new operations.
- Native/process/SSH acceptance or discovered resource/deadline failures.

## Related Docs

- [Session protocol](session-protocol.md)
- [Shelf core](shelf-core.md)
- [Testing](testing.md)
- [Implementation plan](history/implementation-plan.md)
