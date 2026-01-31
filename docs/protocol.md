# Protocol

## Purpose
Document the Phase 1 protocol framing and message types used between:
- client ↔ server proxy (over SSH stdin/stdout)
- server proxy ↔ daemon (over UNIX socket)

## Key Files
- `src/protocol.rs`
- `src/framing.rs`
- `src/proxy.rs`
- `src/daemon.rs`

## Framing
Each request/response is **one-shot** per connection/session.

Wire format:
1. `MAGIC` (4 bytes): `SCB1`
2. `VERSION` (u16, little-endian): `1`
3. `LEN` (u32, little-endian): number of payload bytes
4. `PAYLOAD` (LEN bytes): `bincode`-encoded `Request` or `Response`

## Size Limits
- Default maximum payload size: **10 MiB**
- Enforced when reading frames, and before accepting `Set` in the daemon.

## Message Types

### Request
- `Set { value }`
- `Get`
- `PeekMeta`

### Response
- `Ok`
- `Value { value }`
- `Meta { content_type, size, created_at }`
- `Empty` (means: no value has been set yet)
- `Error { code, message }`

## Clipboard Semantics (Phase 1)
- Only UTF-8 text is supported.
- Content type must be exactly: `text/plain; charset=utf-8`
- An **empty clipboard** is represented by `Set` with `data = []`.
- An **unset value** is represented by `Empty` on `Get`/`PeekMeta`.

## Timestamps
- `created_at` is Unix epoch milliseconds (UTC) as `i64`.

## Error Codes
Protocol-level error codes (returned in `Response::Error`):
- `invalid_request`
- `payload_too_large`
- `invalid_utf8`
- `daemon_not_running`
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

## Update Triggers
- Any changes to `MAGIC`, `VERSION`, message enums, or size limits.
- Adding binary/image clipboard formats or multi-format metadata.

## Related Docs
- `ARCHITECTURE.md`
- `IMPLEMENTATION_PLAN.md`
