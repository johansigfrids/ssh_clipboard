# SSH Clipboard

This tool is for copying clipboard content between machines on a local network. I has client implementations for Mac and Windows, and a server implementaiton for Linux. 

Hitting a keyboard shortcut copies to current content of the clipboard over to the server. A separate keyboard shortcut copies the content on the server over to the clipboard on the current client.

Communication with the server is done over SSH, and the server does not persist copied content to disk but keeps it in memory. 

## Status
- Linux server: daemon/proxy implemented.
- Clients: `push`/`pull`/`peek` implemented (text + PNG images).
- Hotkeys and background UX are not implemented yet.

## Client CLI (Phase 2)

### Requirements
- The Linux server must be running `ssh_clipboard daemon`.
- The `ssh_clipboard` binary must be available on the server `PATH` for the SSH user (so `ssh user@server ssh_clipboard proxy` works).
- The client uses the system `ssh` binary.

### Examples
Push clipboard to server:
```
ssh_clipboard push --target user@server
```

Pull clipboard from server:
```
ssh_clipboard pull --target user@server
```

Push from stdin:
```
cat note.txt | ssh_clipboard push --stdin --target user@server
```

Pull to stdout (instead of clipboard):
```
ssh_clipboard pull --stdout --target user@server
```

Pull PNG image to a file:
```
ssh_clipboard pull --output ./clipboard.png --target user@server
```

Pull binary/image as base64:
```
ssh_clipboard pull --stdout --base64 --target user@server
```

Peek metadata:
```
ssh_clipboard peek --target user@server
```

Peek metadata (JSON):
```
ssh_clipboard peek --json --target user@server
```

Peek metadata via pull:
```
ssh_clipboard pull --peek --target user@server
```

### Common options
- `--target user@host` (preferred) or `--host host --user user`
- `--port 2222` (recommended; `user@host:2222` may work for simple hostnames)
- `--identity-file ~/.ssh/id_ed25519`
- `--ssh-option <opt>` (repeatable; passed as `ssh -o <opt>`)
- `--timeout-ms 7000`
- `--max-size <bytes>` (default 10 MiB)

## Agent Mode (Phase 4 - hotkeys + tray)
The background agent (tray icon + global hotkeys) is behind the Cargo feature `agent`.

Build and run:
```
cargo run --features agent -- agent
```

Useful commands:
```
ssh_clipboard config path
ssh_clipboard config show --json
ssh_clipboard config validate
ssh_clipboard autostart status
ssh_clipboard autostart enable
```

Notes:
- The agent reads its settings from the config file (see `ssh_clipboard config path`).
- Set `target` (e.g. `user@server`) in the agent config before running, otherwise `config validate` will fail.

## Linux Daemon/Proxy (Phase 1)

### Build
```
cargo build
```

### Run the daemon
```
ssh_clipboard daemon
```

### Run the proxy (invoked via SSH)
```
ssh user@server ssh_clipboard proxy
```

### Defaults
- Socket path: `$XDG_RUNTIME_DIR/ssh_clipboard/daemon.sock` or `/tmp/ssh_clipboard-$UID/daemon.sock`
- Max payload: 10 MiB
- One-shot request/response per SSH session
- UTF-8 text only (`text/plain; charset=utf-8`)

### Options
- `--socket-path <path>`
- `--max-size <bytes>`
- `--io-timeout-ms 7000`
