# Shelf Session Protocol

## Purpose

Freeze the new endpoint's bootstrap/framing and initial message contract before daemon integration. Version 2 (`SCB1`) remains a separately selected legacy endpoint. This document does not make a new command available or change an installed service.

## Key Files

- `src/session_protocol.rs`: typed messages, decimal integers, and negotiation.
- `src/session_protocol/framing.rs`: bounded metadata/raw payload frames.
- `src/session_protocol/tests.rs`: fixtures and malformed/boundary probes.
- `src/shelf.rs`: validated payload ownership and shelf mutations.
- `src/protocol.rs`, `src/framing.rs`: frozen legacy version 2, unchanged.

## Endpoint and Bootstrap

`ssh_clipboard session-proxy` is the new experimental Linux SSH endpoint, backed by a separate owner-only session socket in the daemon's runtime directory. The existing `proxy` command/forced-command key continues to mean version 2. Never probe this endpoint by sending new serialized enums to `proxy`, silently change a forced command, or upload before negotiation. The [native endpoint](shelf-server.md) is implemented and tested in WSL; client/cutover work remains pending.

Every frame has a fixed 16-byte header, followed by bounded UTF-8 JSON metadata and then raw payload bytes:

| Offset | Bytes | Meaning |
| --- | --- | --- |
| 0 | 4 | ASCII `SCBS` |
| 4 | 2 | Bootstrap/framing version `1`, little-endian u16 |
| 6 | 2 | Reserved flags, zero |
| 8 | 4 | Metadata byte length, little-endian u32 |
| 12 | 4 | Raw payload byte length, little-endian u32 |
| 16 | metadata length | One JSON control object |
| following | payload length | Exact encoded representation bytes, without base64 |

The bootstrap envelope is independent of negotiated session versions. Unknown framing versions/flags fail closed; a future incompatible bootstrap needs an explicitly separate entry point. Header sizes are checked before allocation. Metadata and encoded payload budgets are separate: a maximum-sized item does not lose space to JSON metadata. No clipboard payload is embedded in JSON.

The first frame is Hello, with at most 4,096 metadata bytes and no payload. It supplies a nonzero request ID, 1-8 distinct supported session versions, and positive client encoded/decoded limits. Initial session version is `1`; lack of overlap yields `unsupported_version`, without fallback upload. Welcome echoes the request ID and reports the selected version, server version/epoch/sequence, capabilities, effective item/decoded limits, fixed 65,536-byte session metadata bound, retention/storage/resident policies, retry record/time bounds, and a maximum 16-item list page. Effective item and decoded limits are the minimum of client/server policy. Clients implementing version 1 must accept the fixed metadata bound.

Initial capabilities are `list`, `fetch`, `delete`, `clear`, and `deduplicated_share`. Health is always available. Subscription/reconnect/cancellation capability is not advertised yet. Metadata field/type changes require negotiated compatibility; unknown fields/types are rejected. New optional capability names can be advertised without making old clients use them.

UUIDs serialize as hyphenated strings. Request IDs, sequences, and signed millisecond timestamps serialize as canonical decimal **strings**, preserving full integer precision through JavaScript/IPC. Numeric policy fields are bounded u16/u32 JSON numbers. JSON numbers, leading zeroes, `+` signs, negative zero, and out-of-range decimal strings are rejected for string-integer fields. Request ID zero is reserved for uncorrelated protocol errors; client requests use nonzero IDs and the future session driver must reject reuse while outstanding.

## Initial Messages

Messages are internally tagged JSON objects with a snake-case `type`. Each request carries `request_id`. After Welcome, all requests carry `epoch` and must match the active server epoch. Responses echo request IDs and report current epoch/sequence; success requires both correlation and the expected response kind. Error frames carry a stable code and optional epoch, without arbitrary server diagnostic strings.

| Client type | Other metadata | Raw payload | Server type |
| --- | --- | --- | --- |
| `hello` | `versions`, `max_item_bytes`, `max_decoded_bytes` | none | `welcome` |
| `share` | epoch, operation UUID, origin, representation, optional captured timestamp | exactly representation `encoded_bytes` | `shared` |
| `list` | epoch, offset, limit (1-16), optional expected sequence | none | `listed` |
| `fetch` | epoch, item UUID, representation ID | none | `value` |
| `delete` | epoch, item UUID | none | `deleted` |
| `clear` | epoch | none | `cleared` |
| `health` | epoch | none | `health` |

Origin has device UUID/name, subject to the shelf's 256-byte/no-control rules. A representation has ID, content type, encoded byte count, and optional dimensions. Initial ID is `0`; only UTF-8 text and PNG are supported. Initial items have one representation descriptor; multiple simultaneous representations require a future negotiated capability and verified native adapters. Supplied PNG dimensions are untrusted and must agree with decoded content when present; text must not claim dimensions. The server derives the stored descriptor from validated bytes. PNG validation must enforce effective decoded limits, not only global storage policy.

Item metadata has item ID, creation sequence, optional origin/captured timestamp, server receipt/expiry display timestamps, and representation descriptors. `value` identifies the representation whose exact bytes follow. Metadata must agree with header/body length; a body on a control-only message is rejected before upload allocation. Empty text shares still have an explicit zero-length upload and remain distinct from missing items.

List returns newest-first metadata with offset, total, and optional next offset. The first page returns its sequence; later pages must include that expected sequence. A concurrent change returns `resync_required`, so the client restarts enumeration instead of combining pages from different shelf states. Pages are bounded at 16 items; exact serialized metadata must also fit the envelope. Fetch/list responses never write the local clipboard themselves.

Shared replies return item metadata, `duplicate`, and `available`. Identical retained retries return the original item even after deletion/eviction, with `available=false`; they never resurrect it. See [shelf retry guarantees](shelf-core.md#retry-guarantees). A changed epoch or expired retry guarantee requires explicit reconciliation/uncertainty, not automatic capture/upload.

Stable errors: `invalid_request`, `unsupported_version`, `invalid_state`, `payload_too_large`, `invalid_content`, `busy`, `retry_window_full`, `operation_conflict`, `epoch_changed`, `not_found`, `resync_required`, `daemon_unavailable`, and `internal`.

Stored PNG descriptors also include `decoded_bytes`, the larger of native decoder output and RGBA pixel bytes. This prevents sending a 16-bit image to a peer whose budget fits its dimensions as RGBA but cannot hold its native decode buffer. Text uses null. A Share may omit the field when unknown; supplied dimensions/decoded counts must agree with validation. This completes the unreleased initial descriptor before endpoint activation; it does not change Hello or legacy fixtures.

## Resource and Session Rules

The frame reader separates header, metadata, and payload stages. Decode/validate the control object and body association before reserving raw upload bytes. Read directly into the shelf's bounded upload buffer. Framing/decode failure closes the stream. A resource rejection may drain only its validated bounded body under the operation deadline before replying; never continue from an undrained body. Send payloads from immutable leases without duplicating their bytes into JSON or a combined frame buffer.

Future endpoint integration must bound accepted connections and queued work before allocation, apply operation/idle/write deadlines, enforce same-UID/socket ownership, release leases on disconnect, and run expiry without incoming traffic. It must implement explicit hello/ready/closed state and validate per-peer limits, epoch, IDs, response kind, pagination, and representations. No claim of a usable persistent session follows from framing tests alone.

New framing is strict initially; bounded noisy-shell resynchronization belongs to the client session driver and must never scan payload bytes as new messages after a framing failure. Legacy one-shot noisy-shell support remains unchanged.

## Evidence and Remaining Work

The codec/negotiation fixtures are tested independently of OS sockets and SSH. Version 2 fixtures stay in their existing module. The [transport-independent service](shelf-service.md) now has duplex lifecycle/action tests, including pagination and deadlines. [Native Linux daemon/proxy/process checks](shelf-server.md) now pass in WSL. Forced-command compatibility/cutover, client integration, and a new real SSH endpoint test remain pending.

## Update Triggers

- Bootstrap bytes, session version, capability, message field, or rejection behavior changes.
- Endpoint/session integration and native acceptance evidence lands.

## Related Docs

- [Legacy protocol](protocol.md)
- [Shelf core](shelf-core.md)
- [Implementation plan](implementation-plan.md)
- [Connection diagnostics](connection-diagnostics.md)
