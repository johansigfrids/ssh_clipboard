# Server Setup (Linux)

## Purpose
Explain how to run the daemon on the Linux server and how the SSH-invoked proxy interacts with it.

## Key Files
- `src/main.rs`
- `src/daemon.rs`
- `src/proxy.rs`
- `README.md`

## Build
Preferred: download a Linux server release artifact and put `ssh_clipboard` on your `PATH`.

Build from source on the Linux server:
```
cargo build --release --no-default-features
```

The resulting binary is `target/release/ssh_clipboard`.

## Run the Daemon
Run as the target user (per-user daemon):
```
ssh_clipboard daemon
```

Optional timeouts:
```
ssh_clipboard daemon --io-timeout-ms 7000
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

Optional timeouts:
```
ssh user@server ssh_clipboard proxy --io-timeout-ms 7000
```

Optional auto-start (off by default):
```
ssh user@server ssh_clipboard proxy --autostart-daemon
```

Ensure `ssh_clipboard` is on the server `PATH` for that SSH user, or invoke it via an absolute path.

## Client Usage (Windows/macOS/Linux)
From a client machine, use:
```
ssh_clipboard push --target user@server
ssh_clipboard pull --target user@server
ssh_clipboard peek --target user@server
```

The client invokes the proxy using the system `ssh` binary (with `ssh -T`) and speaks the framed protocol over stdin/stdout.

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
- Changes to proxy auto-start behavior (`--autostart-daemon`).

## Related Docs
- `docs/protocol.md`
- `ARCHITECTURE.md`
