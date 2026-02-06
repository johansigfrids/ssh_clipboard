# ssh_clipboard

`ssh_clipboard` copies clipboard content between machines over SSH.

- Clients: Windows / macOS / Linux
- Server: Linux (daemon + proxy)

The Linux server keeps the latest clipboard value **in memory only** (no on-disk persistence).

## Quick Start (recommended)

Download the release artifact that matches your platform and CPU architecture.
- macOS: `x86_64` = Intel, `aarch64` = Apple Silicon
- If the binary is not on `PATH`, run it from the extracted folder:
  - macOS/Linux: `./ssh_clipboard`
  - Windows PowerShell: `.\ssh_clipboard.exe`

### 1) Linux server (daemon + proxy)
1) Download a Linux release artifact on the server and extract it (example folder: `~/ssh_clipboard`).

2) Run the one-command setup (Ubuntu/systemd user service):
```
./ssh_clipboard install-daemon
```

This will:
- symlink `/usr/local/bin/ssh_clipboard` → `./ssh_clipboard` (uses `sudo`)
- write `./ssh_clipboard.service`
- symlink it into `~/.config/systemd/user/ssh_clipboard.service`
- enable + start the service

Important:
- Do not move or delete the extracted folder after install; rerun `install-daemon` if you do.

3) Verify the proxy works over SSH:
```
ssh -T user@server ssh_clipboard proxy --help
```

### 2) Client (agent: tray + hotkeys)
1) Make sure SSH works non-interactively (recommended for hotkeys):
```
ssh user@server true
```

2) One-command client install + setup (recommended for first-time use):
```
# macOS/Linux
./ssh_clipboard install-client --target user@server

# Windows PowerShell
.\ssh_clipboard.exe install-client --target user@server
```
This installs `ssh_clipboard` + `ssh_clipboard_agent` to a stable user-local location, updates your user PATH, runs `setup-agent`, verifies autostart, runs `doctor`, and starts the agent once.
To remove the client install later, run `ssh_clipboard uninstall-client`.

3) For later sessions, use from PATH:
```
ssh_clipboard agent
```

Notes:
- Linux hotkeys require X11; on Wayland use `ssh_clipboard agent --no-hotkeys`.
- If tray init fails, run `ssh_clipboard agent --no-tray`.
- Autostart uses the `ssh_clipboard_agent` binary (bundled alongside `ssh_clipboard`) to avoid opening a terminal window on Windows/macOS.

## Essentials (daily usage)

### CLI (ad hoc / scripts)
Push clipboard to server:
```
ssh_clipboard push --target user@server
```

Pull clipboard from server:
```
ssh_clipboard pull --target user@server
```

Peek metadata:
```
ssh_clipboard peek --target user@server
```

### Input/output modes
Push text from stdin:
```
cat note.txt | ssh_clipboard push --stdin --target user@server
```

Pull to stdout (instead of clipboard):
```
ssh_clipboard pull --stdout --target user@server
```

Pull PNG to a file:
```
ssh_clipboard pull --output ./clipboard.png --target user@server
```

### Robust defaults
- The client tolerates “noisy shells” / MOTD bytes before the protocol by resyncing frames by default.
  - Use `--strict-frames` to disable resync and fail fast instead.
- Default max payload is **10 MiB** (`--max-size`).

## Feature overview
- Clipboard formats: UTF-8 text and PNG images
- One-shot request/response per SSH session
- Agent: tray menu (Push/Pull/Peek), global hotkeys, notifications, optional autostart
- Server: per-user daemon, UNIX socket permissions, peer credential checks (Linux)

## More help (user docs)
This repo’s `docs/` folder is intended for internal/dev documentation.

For user-facing help, rely on CLI help:
```
ssh_clipboard --help
ssh_clipboard <command> --help
```

Useful starting points:
- `ssh_clipboard doctor --target user@server`
- `ssh_clipboard install-client --help`
- `ssh_clipboard uninstall-client --help`
- `ssh_clipboard setup-agent --help`
- `ssh_clipboard install-daemon --help`
- `ssh_clipboard push --help`
- `ssh_clipboard pull --help`
- `ssh_clipboard peek --help`

## Developer docs (internal)
If you’re hacking on the project or packaging it, start at `docs/index.md`.

## Build from source
Client build (agent enabled by default):
```
cargo build --release
```

Linux server-only build (no agent/UI deps):
```
cargo build --release --no-default-features
```

## License
MIT. See `LICENSE`.

## Disclaimer
This project is 100% vibecoded and has not had human code review.
