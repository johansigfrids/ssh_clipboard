> Historical 0.4 compatibility guide. The 0.5 source removes these implementations; use the current [fresh setup guide](../../compatibility-and-cutover.md).

# Legacy V2 Protocol

## Purpose
Document the protocol framing and message types used between:
- client ↔ server proxy (over SSH stdin/stdout)
- server proxy ↔ daemon (over UNIX socket)

## Key Files
- `src/protocol.rs`
- `src/framing.rs`
- `src/proxy.rs`
- `src/daemon.rs`

## Framing

This page describes the current version 2 endpoint. The new [shelf core](../../shelf-core.md) is implemented separately and has not changed these serialized enums or endpoint bytes. Its [session/bootstrap protocol](../../session-protocol.md) is integrated with the shelf daemon, client and desktop. V2 and sessions remain explicitly selected endpoints.
Each request/response is **one-shot** per connection/session.

Wire format:
1. `MAGIC` (4 bytes): `SCB1`
2. `VERSION` (u16, little-endian): `2`
3. `LEN` (u32, little-endian): number of payload bytes
4. `PAYLOAD` (LEN bytes): `wincode`-encoded (`bincode`-compatible) `Request` or `Response`

### Resync (noisy shell / MOTD)
If the client receives unexpected bytes before `MAGIC` (e.g., shell banners), it may resync by scanning for the next `MAGIC` sequence and discarding garbage bytes.
This is enabled by default in client reads and can be disabled via `--strict-frames`.

## Size Limits
- Default maximum payload size: **10 MiB**
- Enforced when reading frames, and before accepting `Set` in the daemon.

## Message Types

### Request
Requests include a `request_id` (u64) used for correlation across client/proxy/daemon logs.

- `Request { request_id, kind: Set { value } }`
- `Request { request_id, kind: Get }`
- `Request { request_id, kind: PeekMeta }`

### Response
Responses echo the `request_id` from the corresponding request.

- `Response { request_id, kind: Ok }`
- `Response { request_id, kind: Value { value } }`
- `Response { request_id, kind: Meta { content_type, size, created_at } }`
- `Response { request_id, kind: Empty }` (means: no value has been set yet)
- `Response { request_id, kind: Error { code, message } }`

## Clipboard Semantics
- UTF-8 text (`text/plain; charset=utf-8`) and PNG images (`image/png`) are supported.
- Only **one format at a time** is stored (single `content_type` + `data`).
- Empty payloads are permitted (for example, an empty string).
- An **unset value** is represented by `Empty` on `Get`/`PeekMeta`.

## Timestamps
- `created_at` is Unix epoch milliseconds (UTC) as `i64`.

## Error Codes
Protocol-level error codes (returned in `Response::Error`):
- `invalid_request`
- `payload_too_large`
- `invalid_utf8`
- `daemon_not_running`
- `version_mismatch`
- `internal`

Proxy process exit codes (Linux):
- `0`: success
- `2`: invalid request (includes invalid UTF-8)
- `3`: payload too large
- `4`: daemon not running / socket unavailable
- `5`: internal error

### Proxy exit status vs protocol response
The proxy may exit non-zero while still writing a valid `Response::Error` frame to stdout.
Clients should treat the framed `Response` as the primary signal and use the process exit status only as a secondary hint (or for logging).

The rework one-shot client verifies the request ID and expected reply kind before accepting any response. Stderr is drained concurrently with bounded retention, and one operation deadline covers stream work and process completion. A framed error can survive a nonzero proxy exit; success requires successful completion. These client lifecycle changes preserve the version 2 wire bytes; see [transport guardrails](../../persistent-sessions.md).

## Update Triggers
- Any changes to `MAGIC`, `VERSION`, message enums, or size limits.
- Adding binary/image clipboard formats or multi-format metadata.

## Related Docs
- [ARCHITECTURE.md](../../../ARCHITECTURE.md)
