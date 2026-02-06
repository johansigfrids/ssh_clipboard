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

### `doctor`
Run connectivity diagnostics for SSH/proxy/protocol setup.

Common usage:
```
ssh_clipboard doctor --target user@server
```

Checks include:
- local `ssh` binary availability
- target resolution
- non-interactive SSH auth (`ssh -T ... true`)
- remote proxy command availability (`ssh_clipboard proxy --help`)
- protocol roundtrip (`PeekMeta`)

Flags:
- `--target user@host[:port]` (for simple hostnames; use `--port` for IPv6)
- `--host`, `--user`, `--port`
- `--identity-file <path>`
- `--ssh-option <opt>` (repeatable; passed as `ssh -o <opt>`)
- `--ssh-bin <path>`
- `--timeout-ms <ms>` (default 7000)

Notes:
- If `--target`/`--host` is omitted, `doctor` will try the saved agent config target (when the agent feature is enabled).

### `agent` (Windows/macOS/Linux)
Run the background agent (tray icon + hotkeys).

Flags:
- `--no-tray`: disable tray UI
- `--no-hotkeys`: disable hotkeys

Notes:
- `ssh_clipboard agent` runs in-process when attached to a terminal; otherwise it launches the `ssh_clipboard_agent` binary (matching autostart behavior).
- Linux hotkeys require X11; on Wayland use `--no-hotkeys`.

### `install-client` (Windows/macOS/Linux)
Install client binaries to a stable user-local location, update PATH, run setup, verify, and start the agent.

Common usage:
```
# from extracted release folder
./ssh_clipboard install-client --target user@server
```

Flags:
- `--target <user@host>` (required)
- setup/connectivity flags: `--port`, `--identity-file`, `--ssh-option`, `--clear-ssh-options`, `--max-size`, `--timeout-ms`, `--resync-frames`, `--resync-max-bytes`
- `--install-dir <path>`: override default install directory
- `--no-path-update`: skip PATH persistence changes
- `--no-start-now`: skip immediate agent launch after install
- `--dry-run`: print planned actions only
- `--force`: overwrite existing binaries in the install directory

Defaults:
- install directory:
  - Windows: `%LOCALAPPDATA%\\ssh_clipboard\\bin`
  - macOS/Linux: `~/.local/bin`
- verifies autostart and runs `doctor` after setup
- remote verification failures are warnings (local install still succeeds)

### `uninstall-client` (Windows/macOS/Linux)
Remove binaries and PATH/autostart integration created by `install-client`.

Common usage:
```
ssh_clipboard uninstall-client
```

Flags:
- `--install-dir <path>`: override default install directory
- `--no-path-cleanup`: skip PATH cleanup
- `--dry-run`: print planned actions only
- `--force`: continue on non-critical cleanup errors

Notes:
- Keeps agent config/log files by default.
- On Windows, uninstall attempts to stop running `ssh_clipboard_agent.exe` processes before removing binaries.
- If a Windows binary is still in use, removal is best-effort deferred.

### `setup-agent` (Windows/macOS/Linux)
One-command setup for the agent: writes config (sets target) and enables autostart.

Common usage:
```
ssh_clipboard setup-agent --target user@server
```

Flags:
- `--no-autostart`: do not enable autostart (disables if already enabled)
- `--dry-run`: print the resulting config and planned actions without changing the system
- accepts the same connection options as `config set` (port/identity/ssh-option/etc.)

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

### `install-daemon` (Linux only)
Set up the daemon and systemd user service in one command.

Common usage:
```
./ssh_clipboard install-daemon
```

Flags:
- `--dry-run`: print actions and unit contents without changing the system
- `--force`: overwrite existing unit source/link
- `--no-sudo`: do not use sudo (fails if `/usr/local/bin` cannot be updated)
- `--max-size <bytes>`
- `--io-timeout-ms <ms>`
- `--socket-path <path>`

### `uninstall-daemon` (Linux only)
Remove the systemd user service and PATH symlink created by `install-daemon`.

Common usage:
```
./ssh_clipboard uninstall-daemon
```

Flags:
- `--dry-run`: print actions without changing the system
- `--no-sudo`: do not use sudo (fails if `/usr/local/bin` cannot be removed)

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
