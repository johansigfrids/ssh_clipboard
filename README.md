# ssh_clipboard

`ssh_clipboard` copies clipboard content between machines over SSH.

- Clients: Windows / macOS / Linux
- Server: Linux (daemon + proxy)

The Linux server keeps the latest clipboard value **in memory only** (no on-disk persistence).

## Desktop rework prototype

The rework branch includes an experimental quick panel in `desktop/`: explicit share/copy, content previews, a panel shortcut, tray access, and settings saved on this computer. It uses the existing Linux server and leaves the installed agent/configuration alone.

From `desktop/`, with Node.js 24, Rust, and [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/):

```powershell
npm ci
npm run tauri dev
```

Enter your SSH alias or `user@host` in Settings, apply it, test the connection, then refresh or share. Copy writes the displayed item to your local clipboard. Close hides the panel; Quit exits the app.

The prototype defaults to the legacy 10 MiB transfer limit, including request metadata, and honors a selected profile's limit. Images use lossless PNG compression with a separate 64 MiB pixel limit. Size errors explain what exceeded the limit; rejected shares keep the previous item. A client override does not reconfigure the server; larger negotiated limits are part of the coordinated client/server rework.

This is a source-build prototype with persistent settings stored separately from the old agent. Connection settings support named profiles: add, rename, choose and Apply, or remove with confirmation. A collapsed Advanced section edits per-profile SSH options, executable, timeout, transfer limit, and framing recovery. Test connection shows which steps passed or failed and what to check next. Optional direct Share/Copy shortcuts have recorders and explicit opt-in; see [shortcut behavior](docs/desktop-shortcuts.md). The rework CLI can inspect/edit them through `ssh_clipboard settings`; transfers without an explicit destination use the selected profile. The desktop reloads CLI edits automatically and protects unsaved drafts. Legacy/prototype settings migrate with exact backups; old startup registrations remain untouched. Behavior settings choose panel, private notification, or quiet feedback for direct actions. Portable Windows builds use panel fallback for requested notifications. Shelf profiles now update history metadata automatically and recover transient network failures with bounded backoff. Authentication/trust errors pause with an explicit Reconnect history action; recovery never shares or copies automatically. Active operations expose Cancel. An uncertain shelf share can be checked or explicitly retried from its original capture within a server-enforced window; recovery never reads a newer clipboard. See [share recovery](docs/share-recovery.md). Installed-build staging now bundles a matching CLI in a signed per-user package. Release builds expose explicit start-at-login controls; portable prototypes cannot register startup. A synthetic Windows package has been built and verified without installation. Installed builds also have bounded quiet update checks, explicit verified downloads, and guarded install confirmation. Production trust setup and installed acceptance remain in progress; see [package/startup evidence](docs/desktop-updates.md). See the [prototype guide](docs/desktop-prototype.md), [shared settings](docs/shared-configuration.md), [migration/recovery](docs/configuration-migration.md), and [implementation plan](docs/implementation-plan.md).

The Linux rework also includes an experimental bounded shelf server and session proxy. An optional V2 compatibility socket lets existing clients use its latest item while new sessions access history. The explicit `shelf` CLI now lists metadata, shares file/stdin content, exports selected items, and guards delete/clear by server epoch; see [client commands](docs/shelf-client.md). Connection settings can now opt into [desktop history](docs/desktop-history.md), including selection, safe copy, and confirmed delete/clear. Existing profiles keep V2 transfers. The server requires an explicit cutover; see [endpoint commands and compatibility](docs/shelf-server.md).

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
- Linux on Wayland auto-disables global hotkeys at startup with a single notice; use tray actions, or run under X11 to enable hotkeys.
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
