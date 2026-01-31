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

### Phase 2 Goals
- Provide a usable CLI on Windows and macOS to `push`/`pull` clipboard text via the Linux server daemon (through the SSH proxy).
- Keep the one-shot model: **one request/response per SSH invocation**.
- Enforce the 10 MiB payload size limit client-side before sending.
- Keep Phase 2 text-only (images/binary deferred to Phase 3).

### 1. CLI shape (cross-platform)
- Extend the top-level CLI to include:
  - `push` (client: local clipboard → server)
  - `pull` (client: server → local clipboard)
  - `peek` (optional: show metadata only, via `PeekMeta`)
  - Keep existing Linux-only subcommands:
    - `daemon` (Linux only)
    - `proxy` (Linux only)
- Ensure the binary runs on Windows/macOS and does not hard-exit (current Phase 1 behavior) once `push/pull` exist.
- Add flags for stdin/stdout behavior:
  - `push --stdin` to read text from stdin instead of clipboard.
  - `pull --stdout` to print to stdout instead of writing to clipboard (default remains clipboard).

### 2. SSH invocation layer
- Implement a `client::ssh` module that spawns the platform SSH client using `tokio::process::Command`.
- Default remote command: `ssh_clipboard proxy` (invoked on the Linux server).
- Use `ssh -T` (no TTY) to ensure the binary protocol is not corrupted.
- Capture stderr and surface it on failures (especially for SSH auth/known_hosts issues).
- Provide a clean configuration surface:
  - `--target user@host[:port]` (full target string accepted)
  - `--host` + `--user` + `--port` (optional explicit fields)
  - `--port`
  - `--identity-file`
  - `--ssh-option <opt>` (repeatable; passed as `-o opt`)
  - Optional: `--ssh-bin` to override the ssh executable path
- Security defaults:
  - Do not disable host key checking by default.
  - Do not enable agent forwarding by default; rely on user SSH config if needed.

### 3. Client protocol send/receive
- Implement `client::transport` helpers:
  - Build `Request` (`Set/Get/PeekMeta`)
  - `bincode` encode + frame write to SSH stdin
  - frame read + decode `Response` from SSH stdout
- Handle these failure classes distinctly:
  - SSH spawn/exec failure (local error)
  - SSH exited non-zero (include stderr)
  - Protocol framing errors (invalid magic/version/EOF)
  - `Response::Error` (map server-side codes to client exit codes/messages)
- Timeouts (Phase 2 minimum):
  - Add a default timeout per operation (e.g., 5–10s) and make it configurable (`--timeout-ms`).

### 4. Clipboard integration (Windows/macOS)
- Implement `client::clipboard` using `arboard`:
  - `read_text()` for `push`
  - `write_text()` for `pull`
- If `push --stdin` is provided, read from stdin and skip clipboard access.
- If clipboard read fails and `--stdin` is not set, return a clipboard error.
- If `pull --stdout` is provided, print text to stdout and skip clipboard write.
- On `push`, validate:
  - UTF-8 text only (already in Rust `String`)
  - size ≤ 10 MiB once encoded as UTF-8 bytes
- Populate `ClipboardValue`:
  - `content_type = text/plain; charset=utf-8`
  - `data = text.into_bytes()`
  - `created_at = now_utc_epoch_millis()`

### 5. User experience / exit codes
- Define client exit codes that are stable for scripting, for example:
  - `0` success
  - `2` invalid request/response (protocol or server error `invalid_request`)
  - `3` payload too large
  - `4` daemon not running (proxy exit code passthrough or `Response::Error` if present)
  - `5` SSH failure (auth/hostkey/network)
  - `6` clipboard read/write failure
- Print a short, actionable stderr message per failure (don’t dump huge debug output by default).
- Add `--verbose` to increase logging (`RUST_LOG` support via `tracing-subscriber` env filter).

### 5a. CLI examples (Phase 2)
- Push clipboard to server:
  - `ssh_clipboard push --target user@server`
- Pull clipboard from server:
  - `ssh_clipboard pull --target user@server`
- Push from stdin:
  - `cat note.txt | ssh_clipboard push --stdin --target user@server`
- Pull to stdout:
  - `ssh_clipboard pull --stdout --target user@server`
- Peek metadata:
  - `ssh_clipboard peek --target user@server`

### 6. Tests (pragmatic)
- Unit tests:
  - request/response encode/decode round-trips
  - max-size enforcement on the client path
  - timestamp conversion
- Linux-only integration test (when running CI on Linux later):
  - start daemon on a temp socket
  - invoke proxy directly (without SSH) using the same framing
  - exercise `push` and `pull` logic by swapping the SSH transport for a “local proxy transport”

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
