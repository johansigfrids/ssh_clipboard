# CLI Reference

## Purpose
Concise command/flag reference for the `ssh_clipboard` CLI.

## Key Files
- `src/main.rs`
- `README.md`

## Commands

### `push`
Send local clipboard to server (text or PNG).

Common usage:
```
ssh_clipboard push --target user@server
```

Flags:
- `--stdin`: read text from stdin instead of clipboard
- `--target user@host[:port]` (for simple hostnames; use `--port` for IPv6)
- `--host`, `--user`, `--port`
- `--identity-file <path>`
- `--ssh-option <opt>` (repeatable; passed as `ssh -o <opt>`)
- `--ssh-bin <path>`
- `--timeout-ms <ms>`
- `--max-size <bytes>`
- `--strict-frames`: disable framing resync (strict MAGIC at byte 0)
- `--resync-max-bytes <bytes>`: max bytes to discard before MAGIC (default 8192)

### `pull`
Fetch from server and write to clipboard (default), or output to stdout/file.

Common usage:
```
ssh_clipboard pull --target user@server
```

Flags:
- `--stdout`: print text to stdout
- `--output <file>`: write raw payload to file (PNG or text)
- `--base64`: print binary/image as base64 (requires `--stdout`)
- `--peek`: metadata-only (like `peek`)
- `--json`: with `--peek`, print JSON output
- SSH + timeout + size flags (same as `push`)
- `--strict-frames`, `--resync-max-bytes` (same as `push`)

### `peek`
Fetch metadata only (no payload).

Common usage:
```
ssh_clipboard peek --target user@server
```

Flags:
- `--json`: output JSON (default output is human-readable)
- SSH + timeout + size flags (same as `push`)
- `--strict-frames`, `--resync-max-bytes` (same as `push`)

### `agent` (Windows/macOS/Linux)
Run the background agent (tray icon + hotkeys).

Flags:
- `--no-tray`: disable tray UI
- `--no-hotkeys`: disable hotkeys

Notes:
- Linux hotkeys require X11; on Wayland use `--no-hotkeys`.

### `config`
Manage agent configuration.

Subcommands:
- `config path`
- `config show [--json]`
- `config validate`
- `config defaults`
- `config set --target user@host [--port 2222] [--identity-file <path>] [--ssh-option <opt>] [--clear-ssh-options] [--max-size <bytes>] [--timeout-ms <ms>] [--resync-frames <bool>] [--resync-max-bytes <bytes>]`

### `autostart`
Manage “start at login” for the agent.

Subcommands:
- `autostart enable`
- `autostart disable`
- `autostart status`
- `autostart refresh`

### `daemon` (Linux only)
Run the per-user daemon that stores clipboard contents in memory.

Flags:
- `--socket-path <path>`
- `--max-size <bytes>`
- `--io-timeout-ms <ms>`

### `proxy` (Linux only)
Run the proxy (invoked over SSH).

Flags:
- `--socket-path <path>`
- `--max-size <bytes>`
- `--io-timeout-ms <ms>`
- `--autostart-daemon`: attempt to start the daemon if the socket is unavailable

## Exit Codes (client)
- `0`: success
- `2`: invalid request/response or unsupported content
- `3`: payload too large
- `4`: daemon not running / socket unavailable
- `5`: SSH failure
- `6`: clipboard read/write failure

## Related Docs
- `docs/client-setup.md`
- `docs/linux-client.md`
- `docs/server-setup.md`
- `docs/protocol.md`
