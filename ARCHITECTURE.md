# SSH Clipboard — Architecture

## Purpose
Describe the architecture for `ssh_clipboard`: a cross-platform Rust tool to copy clipboard contents between:
- Windows client ↔ Linux server
- macOS client ↔ Linux server
- Linux client ↔ Linux server

All data transfer occurs over SSH. The Linux server does not persist clipboard contents to disk; it only keeps the latest value in memory while the server daemon is running.

## Goals
- Reliable push/pull of clipboard contents between a client machine and a server machine.
- Transport security via SSH (reuse existing SSH keys/agent/config).
- Server-side clipboard state held in memory only (no on-disk persistence).
- Simple operational model: a small daemon on the Linux server, and a client app on Windows/macOS/Linux.
- User-friendly defaults: the agent is enabled by default for client builds.

## Non-goals (initially)
- Multi-user sharing or access control beyond SSH user isolation.
- Sync/streaming clipboard updates automatically (initially event-driven: push/pull).
- Rich clipboard semantics beyond the current minimal set (e.g., HTML/RTF, multiple simultaneous formats).

## High-Level Design
The system uses a **Linux daemon** that holds clipboard state in memory, plus a **remote proxy mode** that is invoked over SSH to talk to the daemon.

This yields a clean separation:
- **SSH is the only network exposure** (no public TCP listener needed).
- The daemon is reachable only from the same Linux host (via a local UNIX socket with strict permissions).

### Components
1. **Server daemon (Linux)**
   - Long-lived process that stores the most recent clipboard payload in memory.
   - Listens on a local UNIX domain socket.
2. **Server proxy (Linux, runs per SSH session)**
   - A small mode of the same binary (or a separate binary) executed via `ssh user@host ...`.
   - Bridges stdin/stdout (SSH channel) to the daemon UNIX socket.
3. **Client app (Windows / macOS / Linux)**
   - Reads local clipboard, sends it to server (push).
   - Receives clipboard from server, writes it to local clipboard (pull).
   - Uses the platform `ssh` binary and communicates via stdin/stdout frames.
   - Supports both CLI-triggered push/pull and a background agent (tray + global hotkeys).
   - The agent runs as a separate `ssh_clipboard_agent` binary to avoid console windows on Windows/macOS.

## Data Flow

### Push (client → server)
1. Client reads local clipboard into `(content_type, bytes)`.
2. Client spawns `ssh` and runs the remote proxy mode, e.g.:
   - `ssh user@server ssh_clipboard proxy`
3. Client sends a framed `Set` request over the SSH session’s stdin.
4. Remote proxy forwards the request to the daemon over the UNIX socket.
5. Daemon stores payload in memory, returns `Ok`.
6. Proxy forwards response back to client over stdout; client reports success.

### Pull (server → client)
1. Client spawns `ssh user@server ssh_clipboard proxy`.
2. Client sends a framed `Get` request.
3. Proxy asks daemon for current payload.
4. Daemon returns payload (or `Empty`).
5. Proxy forwards response to client; client writes payload to local clipboard.

## Local IPC on the Server

### UNIX socket location
Prefer `$XDG_RUNTIME_DIR` for a per-user runtime socket (in-memory tmpfs on most distros):
- `$XDG_RUNTIME_DIR/ssh_clipboard/daemon.sock`

Fallback if `XDG_RUNTIME_DIR` is unavailable:
- `$TMPDIR/ssh_clipboard-$UID/daemon.sock` or `/tmp/ssh_clipboard-$UID/daemon.sock`

### Permissions
- Create the directory with `0700` and the socket with `0600` (owner-only).
- Daemon runs as the target user (systemd user service recommended).

## On-the-Wire Protocol (Client ↔ Proxy ↔ Daemon)

### Design requirements
- Binary-safe (clipboard may include arbitrary bytes).
- Framed (so multiple messages can reuse the same stream in the future).
- Versioned (to allow evolution).

### Proposed framing
- `MAGIC` (e.g., 4 bytes) + `VERSION` (u16) for handshake/validation.
- One-shot frame per SSH session:
  - `len: u32` (little endian)
  - `payload: [u8; len]`
- `payload` is a serialized request/response (e.g., `serde` + `bincode` or `postcard`).

### Resync for noisy shells
Clients may encounter extra bytes before `MAGIC` (e.g., MOTD banners). The framing reader can resync by scanning for `MAGIC` and discarding garbage bytes (enabled by default; strict mode is available).

Current implementation details are documented in `docs/protocol.md` (including protocol version and request IDs).

### Message types (logical)
Logical operations:
- `Set` (store a clipboard value: text or PNG)
- `Get` (retrieve the current clipboard value)
- `PeekMeta` (retrieve metadata without the full payload)

Responses:
- `Ok`
- `Value`
- `Meta`
- `Empty`
- `Error`

### Size limits
To avoid memory and denial-of-service issues:
- Impose a configurable maximum payload size (default 10 MiB).
- Reject larger payloads with a clear error.

## Clipboard Semantics

### Content types (initial)
Start with a minimal, robust set:
- `text/plain; charset=utf-8`
- `image/png` (PNG bytes; Phase 3+)

### Later extensions
- macOS: NSPasteboard multiple representations
- Windows: CF_UNICODETEXT plus additional formats
- Images (PNG), HTML, RTF

The protocol should treat clipboard as opaque bytes with a `content_type` plus optional metadata.

## SSH Integration

### Why spawn `ssh` instead of embedding an SSH library (initially)
Using the platform SSH client (OpenSSH) provides:
- Existing key management (agent, keychain, Windows OpenSSH)
- User config support (`~/.ssh/config`, known_hosts)
- Fewer cross-platform crypto/ssh edge cases

The Rust client will spawn `ssh` and communicate over stdin/stdout with the remote proxy mode.

### Hardening options (optional)
On the server, restrict what a key can do by pinning a forced command in `authorized_keys`:
- `command="ssh_clipboard proxy",no-port-forwarding,no-agent-forwarding,no-X11-forwarding ...`

### Daemon access hardening
The daemon verifies peer credentials on the UNIX socket and rejects connections from other users.

## Proposed Rust Project Layout
This repo currently uses `src/main.rs` + `src/lib.rs` modules. The agent feature is enabled by default for client builds, but can be disabled for server-only builds.

- `src/lib.rs`
  - `protocol` / `framing` (message types + framing)
  - `daemon` / `proxy` (Linux server side)
  - `client` (SSH + clipboard adapters)
  - `agent` (tray + hotkeys; feature-gated but default-on for clients)

Conditional compilation:
- `#[cfg(target_os = "linux")]` for daemon unix-socket server
- `#[cfg(target_os = "windows")]` / `#[cfg(target_os = "macos")]` / `#[cfg(target_os = "linux")]` for clipboard/hotkey specifics

## Operational Model

### Linux server
- Run daemon via `systemd --user` (recommended) or as a simple background process.
- Daemon holds the clipboard value in memory until it exits/restarts.
- Proxy can optionally autostart the daemon (useful for casual usage; systemd is preferred for production).

### Windows/macOS/Linux client
- MVP: CLI push/pull commands.
- Background agent (default) registers hotkeys and invokes push/pull.

## Update Triggers
- Changes to protocol framing or message types.
- Changes in SSH invocation or required server setup.
- Changes in clipboard format handling or hotkey behavior.
- Changes in packaging, release, or default feature flags.
