# SSH Clipboard — Implementation Plan

## Technology Decisions (based on research)
- **Clipboard (clients):** `arboard` for cross-platform clipboard access (Windows/macOS/Linux) with text+image support; enable the optional Wayland data-control feature if we later add a Linux client.
- **Hotkeys (future phases):** `global-hotkey` for Windows/macOS/Linux (X11) with explicit event-loop requirements.
- **CLI parsing:** `clap` for robust, standard CLI UX and derive-based parsing.
- **Protocol serialization:** `serde` + `bincode` (v2 with `serde` feature) for compact binary encoding.
- **Async I/O:** `tokio` for UNIX socket server and timeouts.
- **Logging:** `tracing` + `tracing-subscriber` for structured diagnostics.
- **Error handling:** `eyre` as the app-level error/report type (optionally with `color-eyre` for rich reports); use `thiserror` for internal typed errors. Avoid `anyhow` unless we need exact API parity with other code.

### Error Handling Rationale
**Why `eyre` (plus `color-eyre` optionally)?**
- Optimized for application-level error reporting with flexible context and report formatting.
- Works well for CLI/daemon apps where user-facing diagnostics matter more than exposing a stable error type.
- `color-eyre` provides richer, more readable reports with minimal setup.

**Why keep `thiserror`?**
- Strongly-typed internal errors make domain failures explicit and testable.
- Easy conversion into `eyre::Report` when bubbling to the top.

**Why not `anyhow`?**
- Functionally similar to `eyre`, but with fewer report customization hooks.
- Unless we need compatibility with existing `anyhow`-based code, `eyre` is a better fit for richer diagnostics.

## Phase 1 — Core protocol + Linux daemon/proxy

### Phase 1 Decisions
- **Connection model:** one-shot per SSH call (single request/response, then exit).
- **Max payload:** 10 MiB, enforced on both client and server.
- **Clipboard scope:** per-user daemon (no cross-user sharing).
- **Phase 1 formats:** UTF-8 text only; binary/image support deferred to Phase 3.

### 1. Protocol + framing (shared)
- Define message enums: `Set`, `Get`, `PeekMeta`, and responses (`Ok`, `Value`, `Empty`, `Error`).
- Add `content_type`, `data`, and `created_at` fields to `Set/Value`.
- Define `created_at` as Unix epoch milliseconds (`i64`, UTC).
- Implement a versioned framing layer: `MAGIC + VERSION + len(u32) + payload`, where payload is `bincode`-encoded and framing is outside the payload.
- Enforce a configurable max payload size (default 10 MiB) on decode and before send.
- Implement async read/write helpers for framed messages over `tokio::io::AsyncRead/AsyncWrite`.
- Define `Empty` to mean “no value set yet”; an empty clipboard is represented by `Set` with zero-length `data`.
- For Phase 1, require `content_type = text/plain; charset=utf-8` and validate UTF-8.

### 2. Linux daemon
- Store a single `ClipboardState` in memory with last-updated timestamp.
- Bind a UNIX socket in `$XDG_RUNTIME_DIR/ssh_clipboard/daemon.sock` (fallback `$TMPDIR/ssh_clipboard-$UID/daemon.sock` or `/tmp/ssh_clipboard-$UID/daemon.sock`).
- Ensure directory `0700` and socket `0600`; refuse to start if permissions are unsafe; set `umask` defensively.
- Handle connections concurrently (tokio tasks) but each connection processes a single request.
- Define error codes and map them to daemon responses and process exit codes.

### 3. Proxy mode (SSH entrypoint)
- Read a single framed request from stdin, forward to daemon socket, return response to stdout, then exit.
- If daemon is not running, return a specific error code (no auto-start in Phase 1).
- Propagate daemon errors with explicit exit codes and messages.
- Add a short handshake/health check for clearer error reporting.

### 4. CLI wiring
- Add subcommands: `daemon` and `proxy` (Linux-only, behind `cfg(target_os = "linux")`).
- Add `--socket-path` override and `--max-size` configuration (default 10 MiB).
- Document exit codes for `daemon`/`proxy`.

### 5. Minimal tests
- Unit tests for framing encode/decode and size limits.
- Integration test for daemon + proxy happy path (Linux-only).

## Phase 2 — Client CLI (Windows/macOS)
1. Implement client-side SSH invocation (spawn `ssh` and communicate over stdin/stdout).
2. Implement `push` (read local clipboard, send `Set`) and `pull` (send `Get`, write clipboard).
3. Add helpful exit codes and error messages for scripting.

## Phase 3 — Hardening + ergonomics
1. Add timeouts, improved logging, and robust error mapping.
2. Add richer clipboard formats (binary payloads, images) and optional multi-format metadata.
3. Document server setup and example `authorized_keys` forced command.

## Phase 4 — Hotkeys + background UX
1. Add Windows/macOS global hotkeys for push/pull.
2. Add tray/menu UX and auto-start options (optional).

## Phase 5 — Packaging + release
1. Provide systemd user service example for Linux daemon.
2. Document Windows/macOS distribution strategy.
