# SSH Clipboard

This tool is for copying clipboard content between machines on a local network. I has client implementations for Mac and Windows, and a server implementaiton for Linux. 

Hitting a keyboard shortcut copies to current content of the clipboard over to the server. A separate keyboard shortcut copies the content on the server over to the clipboard on the current client.

Communication with the server is done over SSH, and the server does not persist copied content to disk but keeps it in memory. 

## Status
- Phase 1 implemented for Linux daemon/proxy only.
- Windows/macOS clients (push/pull + hotkeys) are not implemented yet.

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
