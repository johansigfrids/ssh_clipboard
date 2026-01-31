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

### Phase 3 Goals
- Make the tool reliable under real-world conditions (SSH hiccups, daemon restarts, large payloads).
- Improve observability without leaking clipboard contents.
- Extend clipboard support beyond text (images/binary), while keeping the protocol compatible.
- Improve operational ergonomics (service setup, clearer errors, safer defaults).

### 1. Reliability hardening
- **Timeouts everywhere:**
  - Client: already has `--timeout-ms`; ensure it covers spawn, send, receive, and wait.
  - Proxy: add a short timeout for daemon connect/read/write (avoid hanging SSH sessions forever).
  - Daemon: add idle connection timeout per socket connection.
- **Graceful daemon restart behavior:**
  - Clear client errors when daemon is restarted (returns `Empty` after restart).
  - Optional: `--wait-for-daemon <ms>` on proxy/client (retry connect briefly before failing).
- **Robust I/O handling:**
  - Better handling of partial reads/writes, unexpected EOF, and invalid frames.
  - Ensure stdin/stdout are flushed appropriately and errors are propagated.
- **Size-limit hygiene:**
  - Keep 10 MiB default, but ensure response overhead is bounded and documented.
  - Add `--max-size` validation (refuse absurd values; require both ends to match).

### 2. Observability and debugging UX
- Switch to consistent `tracing` spans:
  - request id (random u64) per operation (client + proxy + daemon) for correlation
  - timings: ssh spawn time, round-trip time, daemon processing time
- Add `--json` output for `peek` (optional), useful for scripting and debugging.
- Improve error messages:
  - include hints for common SSH failures (known_hosts, auth, command not found)
  - include socket path in daemon/proxy errors
- Ensure we never log clipboard contents by default (only sizes/types).

### 3. Protocol evolution (compat + metadata)
- Make protocol explicitly forward-compatible:
  - reserve fields/variants where helpful
  - keep `MAGIC`/`VERSION` stable; bump `VERSION` only when required
- Versioning policy:
  - Phase 3 bumped to `VERSION = 2` to carry `request_id` for correlation.
  - Clients/servers must match versions; return a clear `invalid_request` error on mismatch.
- Add optional metadata fields:
  - `source` (client hostname/user) for debugging (optional; not required)
  - `format` enumeration (text/png/etc.) once multi-format exists
- Define a request-id field (u64) carried in requests/responses for correlation across client/proxy/daemon logs.
  - Client generates; proxy forwards; daemon echoes in response.
- Add a clearer distinction between transport failures and application errors in docs.

### 4. Rich clipboard formats (images/binary)
- Extend `ClipboardValue` semantics:
  - keep `content_type` and `data` as the core
  - for images: use `image/png` with PNG bytes (portable across OSes)
  - Phase 3 stores **one format at a time** (single `content_type`); multi-format bundles deferred.
- Client support:
  - `push`:
    - prefer text if available; auto-select image when text is absent
    - no user-facing `--format` flag in Phase 3
  - `pull`:
    - if `content_type` is `image/png`, write image to clipboard
    - add `--stdout` behavior for binary:
      - for text, keep current behavior
      - for binary/image, require `--output <file>` by default
      - add `--base64` to print base64 to stdout (explicit opt-in)
      - default when binary and no explicit output flag: return a clear error
    - if client can’t handle a format, fall back when possible; otherwise return a clear error
- Server support:
  - daemon accepts binary payloads once enabled; still enforces max size
  - `PeekMeta` returns content_type and size so clients can decide how to handle

### 4a. Peek-first ergonomics
- Add `pull --peek` (or reuse `peek`) to allow scripts to check `content_type` and size before pulling.
- Document a suggested flow:
  - `peek` → decide handling → `pull`

### 5. Security + operational ergonomics
- Add a shipped systemd user unit example in `docs/server-setup.md` (and/or a `contrib/` folder).
- Expand `docs/security.md` with:
  - recommended `authorized_keys` forced command variants (absolute path)
  - suggested SSH `Match` block examples for the dedicated clipboard user/key
- Consider adding a dedicated server “install” doc section:
  - where to place the binary, how to ensure it’s on PATH for non-interactive SSH.

### 6. Tests
- Unit tests:
  - framing fuzz-ish tests for invalid input (magic/version/length)
  - image payload encode/decode and size limit checks
- Integration tests (Linux-only):
  - daemon + proxy under timeout conditions
  - binary payload round-trip through daemon/proxy without SSH

## Phase 4 — Hotkeys + background UX

### Phase 4 Goals
- Provide an always-running client “agent” that:
  - registers global hotkeys for push/pull (and optionally peek)
  - offers a tray/menu-bar UI for quick actions + status
  - can launch on login (optional)
- Keep the existing one-shot SSH model: hotkey triggers run the same `push`/`pull` logic from Phase 2/3.
- Avoid blocking the UI/event loop thread during network operations.

### Phase 4 Technology Decisions (based on research)
- **Hotkeys:** `global-hotkey` (Windows/macOS; requires an event loop; macOS must run on main thread).
- **Tray/menu:** `tray-icon` (Windows/macOS; supports being driven by a `winit`/`tao`-style event loop; macOS tray icon must be created on the main thread and only once the event loop is running).
- **Event loop:** `tao` for a cross-platform event loop without creating a visible window; create on main thread for portability.
- **Config storage:** `confy` for OS-appropriate config locations with serde-backed structs.
- **Autostart:** `auto-launch` for Windows registry + macOS Launch Agent / AppleScript (prefer Launch Agent).
- **Single-instance:** `single_instance` to avoid multiple background agents running.
- **Agent log files (optional but recommended):** `tracing-appender` rolling file appender.
- **Notifications:** `notify-rust` to show OS notifications (macOS Notification Center, Windows toast) with a fallback to stdout.
  - Note: `notify-rust` has platform-specific feature differences on macOS/Windows; keep notification usage minimal (summary/body) and fall back cleanly if unsupported.

### 0. Cargo features and build hygiene
- Add a Cargo feature: `agent`
  - Make GUI/hotkey/autostart/notification deps optional and enabled only with `--features agent`.
  - Gate agent-only code behind `cfg(feature = "agent")` and `cfg(any(target_os = "windows", target_os = "macos"))`.
- Keep server builds lean:
  - The Linux daemon/proxy should not require GUI deps.

### 1. Introduce an “agent” mode
- Add a new subcommand: `agent` (Windows/macOS initially; Linux optional later).
- `agent` responsibilities:
  - load config
  - enforce single-instance
  - start event loop (main thread)
  - create tray icon + menu
  - register global hotkeys
  - execute push/pull actions on demand
- Provide `agent --no-tray` (hotkeys only) and `agent --no-hotkeys` (tray only) for troubleshooting.

### 2. Configuration model
- Define a `Config` struct (serde) with:
  - server target (`user@host`), optional port
  - optional `ssh_options` list
  - max size / timeouts
  - hotkey bindings (strings parsed by `global-hotkey`, e.g. `"cmdorctrl+shift+KeyC"`)
  - autostart enabled flag
- Add `config_version` for migration (start at `1`).
- Store/load via `confy`.
- Store/load via `confy` (be explicit about which config strategy/version we use so macOS paths are predictable).
- Add CLI helpers:
  - `config path` (print where confy stores it)
  - `config show` (print current config)
  - `config validate` (check hotkey parse + target presence)
  - `config defaults` (print OS-specific defaults)

### 3. Hotkey registration
- Register at least:
  - Push hotkey:
    - macOS default: `CMD + SHIFT + C`
    - Windows default: `CTRL + SHIFT + C`
  - Pull hotkey:
    - macOS default: `CMD + SHIFT + V`
    - Windows default: `CTRL + SHIFT + V`
- Implementation notes:
  - `global-hotkey` requires an event loop on Windows (win32) and main thread on macOS.
  - Avoid hotkeys that are known to be problematic on specific macOS versions; document “pick conservative combos” and allow user override.
- Debounce/rate-limit hotkey triggers to avoid repeated runs when keys repeat.
- If registration fails (already taken), show a tray warning and keep running.
- Add a “Restore defaults” action (tray menu) that resets bindings to OS-specific defaults.

### 3a. macOS permissions and failure handling (research-driven)
- macOS can restrict apps that monitor input devices (Input Monitoring). If the hotkey implementation requires or triggers it, users must allow the app/terminal in:
  - System Settings → Privacy & Security → Input Monitoring
- Expectation: registering global hotkeys should work without special permissions in many setups, but macOS privacy controls vary by approach and OS version.
- If hotkey registration fails or behaves inconsistently:
  - show an OS notification (if enabled) and print to stdout explaining what to try
  - direct the user to check **System Settings → Privacy & Security → Input Monitoring** and **Accessibility**, and enable the terminal/app if required
- Treat “permission required” and “hotkey already taken” as separate error cases with distinct messages.

### 4. Tray/menu UX
- Create a tray icon with a menu:
  - `Push`
  - `Pull`
  - `Peek` (show meta via OS notification and/or stdout)
  - `Open config` / `Show config path`
  - `Start at login` toggle
  - `Quit`
- Forward tray/menu events into the event loop using `EventLoopProxy` (avoid polling loops).
- macOS note: create tray icon only once the event loop is actually running (winit example uses `StartCause::Init`); do equivalent in tao.

### 5. Background execution model
- Keep the event loop responsive:
  - When a hotkey/menu action fires, schedule the work on a tokio runtime (multi-thread), and post a completion event back to the UI thread.
- Prevent overlapping operations:
  - If an operation is running, either queue one pending operation or ignore subsequent triggers with a status message.
- Status feedback:
  - Tray tooltip/menu item reflects “Last push/pull: success/fail + timestamp”.
  - OS notification on success/failure and on peek results (fallback to stdout when notifications aren’t available).

### 6. Autostart (optional, but planned in Phase 4)
- Implement `autostart enable|disable|status` using `auto-launch`:
  - Windows: current-user registry entry by default.
  - macOS: Launch Agent mode with an absolute path.
- Tray menu toggle should call the same code path.
- Handling moved/updated binaries (recommended approach):
  - Always store the current executable path when enabling autostart.
  - On agent startup, if autostart is enabled, verify the autostart entry points to the current executable path.
  - If it does not, automatically refresh/rewrite the entry and notify the user (this covers “binary moved” and most update workflows).
  - If refresh fails because the path is not absolute or no longer exists, disable autostart and notify the user to re-enable it (auto-launch requires an absolute existing `app_path`).
  - Provide `autostart refresh` as an explicit command to rewrite the entry.

### 7. Logging for the agent
- Add a dedicated log sink for `agent` mode:
  - rolling log files with `tracing-appender` (daily or hourly).
  - never log clipboard contents; log only sizes/types and error summaries.

### 8. Testing + manual checklist
- Unit tests:
  - config load/store (confy) with defaults
  - hotkey string parse/validate (no OS registration in unit tests)
  - event dispatch state machine (hotkey/menu → action request)
- Manual checklist (Windows + macOS):
  - agent starts, tray icon appears
  - hotkeys trigger push/pull
  - repeated triggers are debounced
  - autostart enable/disable works
  - quitting unregisters hotkeys and exits cleanly

## Phase 5 — Packaging + release
1. Provide systemd user service example for Linux daemon.
2. Document Windows/macOS distribution strategy.
