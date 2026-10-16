# Bounded Shelf Core

## Purpose

Define the new in-memory shelf's state/resource contracts before wiring it into the daemon and new session protocol. This module now powers the separate experimental shelf endpoint. It is not used by the installed server, legacy version 2 endpoint, or desktop yet.

## Key Files

- `src/shelf.rs`: limits, upload reservations/validation, item leases, expiry, and mutations.
- `src/shelf/tests.rs`: platform-independent behavioral/resource tests.
- `src/protocol.rs`, `src/daemon.rs`: existing single-value version 2 implementation, still unchanged.

## Model and Defaults

Each shelf has a random epoch and a monotonically increasing mutation sequence. Items have random stable IDs, creation sequence, optional device identity/name and captured timestamp, server receipt/expiry display times, content type, encoded size, and PNG dimensions. Device metadata is descriptive; it is not an authentication boundary. Text and PNG remain single-representation items for this first core.

| Policy | Default | Validated maximum |
| --- | --- | --- |
| Stored item count | 20 | 1,024 |
| Retention | 1 hour | 24 hours |
| Encoded item | 64 MiB | 256 MiB |
| Stored encoded bytes | 256 MiB | 1 GiB |
| Resident encoded bytes | 384 MiB | 2 GiB |
| Decoded PNG buffer/pixel budget | 256 MiB | 256 MiB |
| Retry records | 4,096 | 16,384 |
| Retry retention | 10 minutes | 24 hours |

All budgets/counts must be positive. Stored capacity must hold an individual item; resident capacity must hold the stored budget plus one maximum upload. Origin names are nonblank UTF-8, at most 256 encoded bytes, without control characters. Operation/device IDs must be non-nil UUIDs. Empty text is an item; an unset shelf is separate.

## Allocation and Validation Boundary

Obtain an `UploadBudget` handle and call `allocate(length)` **before** reading an announced payload. Length and global resident capacity are checked before allocation. The private buffer exposes only its fixed mutable slice, so input cannot grow it beyond the reservation. Abandoned/rejected uploads release the charge. Do not first allocate a frame-sized `Vec` elsewhere and copy it into this API.

Validate outside the shelf lock. UTF-8 is checked without changing bytes. PNG limits are set on `ImageReader` before constructing the decoder, checked native/RGBA sizes precede output allocation, and pixel decoding must succeed. Keep the original encoded bytes; decoding is validation, not a replacement image. Only one PNG validation can hold its decode permit at a time. Negotiation can narrow item/decode limits through a restricted budget handle sharing the global counter; it cannot enlarge them. Checked arithmetic, allocation errors, invalid content, and busy budgets produce typed errors without evicting stored content.

Stored and fetched items share one immutable, reference-counted buffer. Its charge stays alive until the last lease drops, including after expiry/deletion/eviction. Fetching does not copy a large payload. Uploads can be refused while slow readers pin removed items; removing them from the list does not make their resident bytes disappear.

These are logical buffer bounds, not a measured total-process RSS limit. PNG decoder working memory and its output buffer each use the decode limit; allocator/container overhead and bounded metadata are additional. The server integration still must bound connections, metadata frames, queued work, per-operation deadlines, and all serialization copies, and release fetch leases after socket deadlines. Legacy wincode serialization copies require separate reservations/admission control. No aggregate memory claim should be made from stored-payload limits alone.

## Ordering, Expiry, and Atomic Commit

Ordering uses the server sequence, never client timestamps or list position. Retention uses `Instant`, so wall-clock adjustment cannot prolong an item. Wall-clock receipt/expiry values are for display. Capture `Now` inside the shelf lock after validation; a time captured before an asynchronous wait can otherwise precede a concurrently committed operation. Reversed monotonic inputs are rejected.

Calls expire due items first; a batch expiry is one sequence change. A new commit evicts oldest items until both count and stored-byte policies fit, then commits the new item with one sequence change. Oversized/malformed uploads fail during preparation and cannot evict existing content. Delete/clear change the sequence only when content is removed. Snapshots contain metadata newest first; fetch-by-ID and latest return immutable leases. Sequence/timestamp overflow fails without removing content for the requested mutation.

Expiry is lazy at the core boundary. The native shelf daemon also calls `expire` on a one-second timer, so abandoned shelf contents are released without subsequent requests. No disk persistence is added.

## Retry Guarantees

The new endpoint will require an expected epoch and a device-scoped operation UUID. A prepared upload has a SHA-256 fingerprint over domain-separated, length-delimited content type, origin identity/name, captured timestamp, and payload bytes. Reusing an operation with different captured content/metadata is a conflict. Retry the captured upload with its original metadata, not a new clipboard sample.

An identical retry within the retained record window returns the original item metadata without a new item, sequence, receipt time, or expiry. If that item was deleted/cleared/evicted, `available=false` is returned; it is never resurrected. Restart changes the epoch and rejects automatic retry into a new shelf. Uploads prepared against another shelf's resource budget are also rejected.

Retry records have separate fixed count and monotonic retention limits. When the window fills, reject new deduplicated operations until records expire; do not evict an unexpired record and silently shorten the advertised retry guarantee. Legacy shares have no operation ID and cannot deduplicate. After record expiry or a server restart, a client must reconcile/report uncertainty rather than promise exactly-once behavior or silently retry.

## Evidence and Integration Work

Sixteen tests cover bounded preview labels, unchanged payload bytes, exact Unicode/whitespace bytes, server ordering despite clock skew, count/byte eviction and exact limits, expiry boundaries, deduplication/conflicts/window saturation, deletion/clear without retry resurrection, epochs/foreign reservations, pinned leases, concurrent atomic admission, PNG validation/limits, malformed content, origins, overflow, and empty versus unset. They use synthetic content and no SSH, OS clipboard, or server process.

The separate [bootstrap/wire contract and codec fixtures](session-protocol.md) and a tested [generic stream service](shelf-service.md) now exist. The [native endpoint](shelf-server.md) supplies bounded listener tasks, timer/ownership, stdio forwarding, an optional V2 compatibility listener with budgeted serialization, and WSL process evidence. Legacy frames are compacted in place without releasing their original allocation charge. The [item-aware CLI/shared client](shelf-client.md) now has stream/process evidence. The [desktop history controls](desktop-history.md) now have automated/browser evidence. Still required: new endpoint SSH/native platform acceptance. The installed 10 MiB legacy policy has not changed.

## Update Triggers

- Shelf policies, payload representations, ownership/serialization, or retry semantics change.
- New endpoint/daemon integration, resource measurements, or acceptance evidence lands.

## Related Docs

- [Protocol](protocol.md)
- [Architecture](../ARCHITECTURE.md)
- [Implementation plan](implementation-plan.md)
- [Testing](testing.md)
