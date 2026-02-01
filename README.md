# SSH Clipboard

Disclaimer: This project is 100% vibecoded and has not had human code review.

`ssh_clipboard` copies clipboard content between machines over SSH.

- Clients: Windows / macOS / Linux
- Server: Linux (daemon + proxy)

Press a hotkey to **push** your current clipboard to the server, and another hotkey to **pull** the server clipboard onto your current machine.

Communication happens over the system `ssh` client. The Linux server keeps the latest clipboard value **in memory only** (no on-disk persistence).

## Status
- Linux server: daemon/proxy implemented.
- Clients: `push`/`pull`/`peek` implemented (text + PNG images) on Windows/macOS/Linux.
- Agent (tray + hotkeys): implemented; Linux hotkeys require X11 (Wayland best effort).

## Installation
Download the appropriate release artifact from GitHub Releases and put the `ssh_clipboard` binary on your `PATH`.

## Getting Started (Most Common Setup)
### 1) Linux Server
1. Install `ssh_clipboard` on the server and ensure it is on `PATH` for your SSH user.
2. Start the daemon:
   ```
   ssh_clipboard daemon
   ```
3. Verify the proxy can be invoked over SSH:
   ```
   ssh user@server ssh_clipboard proxy --help
   ```

### 2) Agent Client (Windows/macOS/Linux)
1. Ensure SSH key authentication works non-interactively (recommended for hotkeys):
   ```
   ssh user@server true
   ```
2. Find the agent config path:
   ```
   ssh_clipboard config path
   ```
3. Edit the config to set `target` (for example `user@server`), then validate:
   ```
   ssh_clipboard config validate
   ```
   Minimal config example (TOML):
   ```toml
   target = "user@server"

   [hotkeys]
   push = "CmdOrCtrl+Shift+KeyC"
   pull = "CmdOrCtrl+Shift+KeyV"
   ```
4. Run the agent:
   ```
   ssh_clipboard agent
   ```
5. Use the tray/hotkeys to push and pull clipboard contents.

Notes:
- Linux agent: hotkeys require X11 (Wayland best-effort).
- The client uses the system `ssh` binary.

## CLI Usage (Ad Hoc / Scripts)

### Requirements
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
- `--port 2222`
- `--identity-file ~/.ssh/id_ed25519`
- `--ssh-option <opt>` (repeatable; passed as `ssh -o <opt>`)
- `--timeout-ms 7000`
- `--max-size <bytes>` (default 10 MiB)
- `--strict-frames` / `--resync-max-bytes <bytes>` (noisy shell/MOTD protection)

## Agent Mode (hotkeys + tray)
The agent provides a tray icon + global hotkeys for push/pull.

Run:
```
ssh_clipboard agent
```

Build and run from source:
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
- Linux notes: tray/hotkeys depend on GTK + X11; Wayland support depends on the desktop environment.

## Linux Daemon/Proxy

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

Optional proxy auto-start (starts the daemon if the socket is unavailable):
```
ssh user@server ssh_clipboard proxy --autostart-daemon
```

### Defaults
- Socket path: `$XDG_RUNTIME_DIR/ssh_clipboard/daemon.sock` or `/tmp/ssh_clipboard-$UID/daemon.sock`
- Max payload: 10 MiB
- One-shot request/response per SSH session
- Content types: `text/plain; charset=utf-8` and `image/png`

### Options
- `--socket-path <path>`
- `--max-size <bytes>`
- `--io-timeout-ms 7000`

## Build From Source
Build client (includes agent by default):
```
cargo build --release
```

Build Linux server-only (no agent/UI deps):
```
cargo build --release --no-default-features
```

## License
MIT. See `LICENSE`.
