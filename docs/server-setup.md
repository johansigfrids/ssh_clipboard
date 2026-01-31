# Server Setup (Linux)

## Purpose
Explain how to run the Phase 1 daemon on the Linux server and how the SSH-invoked proxy interacts with it.

## Key Files
- `src/main.rs`
- `src/daemon.rs`
- `src/proxy.rs`
- `README.md`

## Build
On the Linux server:
```
cargo build --release
```

The resulting binary is `target/release/ssh_clipboard`.

## Run the Daemon
Run as the target user (per-user daemon):
```
ssh_clipboard daemon
```

### Socket location
The daemon binds a UNIX socket under:
- `$XDG_RUNTIME_DIR/ssh_clipboard/daemon.sock` (preferred)
- `$TMPDIR/ssh_clipboard-$UID/daemon.sock` or `/tmp/ssh_clipboard-$UID/daemon.sock` (fallback)

Permissions are owner-only (`0700` directory, `0600` socket).

## Use the Proxy over SSH
The proxy is meant to be executed via SSH and will:
1. Read one request frame from stdin
2. Forward it to the daemon over the UNIX socket
3. Write one response frame to stdout
4. Exit

Example:
```
ssh user@server ssh_clipboard proxy
```

## systemd (user service) — Suggested
This project does not ship a unit file yet, but a typical user unit would:
- set `Restart=on-failure`
- ensure `XDG_RUNTIME_DIR` exists (systemd user services typically do)
- run `ssh_clipboard daemon`

## Troubleshooting
- **Proxy says daemon not running:** start the daemon first, or ensure the socket path matches on both ends (`--socket-path`).
- **Permission denied on socket:** ensure daemon and proxy run as the same Linux user.
- **Payload too large:** default max payload is 10 MiB; adjust via `--max-size` (both sides should match).

## Update Triggers
- Changes to socket path logic, permissions, or exit codes.
- Adding auto-start behavior for daemon (not planned for Phase 1).

## Related Docs
- `docs/protocol.md`
- `ARCHITECTURE.md`
- `IMPLEMENTATION_PLAN.md`
