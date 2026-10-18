# Retired Agent (Hotkeys + Tray)

## Purpose
Document the background agent: tray/menu UX, global hotkeys, config, and logs.

## Key Files
- `src/agent/mod.rs`
- `src/agent/run.rs`
- `src/agent/autostart.rs`
- `src/agent/notify.rs`
- `src/main.rs`
- [docs/cli.md](../cli.md)

## Build and Run
The old agent is retired from default builds and new distribution. Use the signed desktop for normal use. This document preserves the explicit `--features agent` compatibility path through 0.4; see [retirement](../legacy-retirement.md). A Linux server-only build still uses `--no-default-features`.

Run from source (agent binary):
```
cargo run --features agent --bin ssh_clipboard_agent
```

You can also run via the CLI wrapper:
```
ssh_clipboard agent
```

## Configuration
The agent uses `confy` and stores its config in an OS-specific location.

Show config path:
```
ssh_clipboard config path
```

Show current config:
```
ssh_clipboard config show --json
```

Set config via CLI:
```
ssh_clipboard config set --target user@server --port 2222
```

One-command setup (recommended):
```
ssh_clipboard setup-agent --target user@server
```

Validate config:
```
ssh_clipboard config validate
```

### Required fields
- `target`: SSH target (e.g., `user@server`). This must be set for the agent to run.

### Transport options
- `resync_frames`: when true, the client can skip noisy bytes before MAGIC (default true).
- `resync_max_bytes`: max bytes to discard before failing (default 8192).

### Hotkey bindings
Bindings are stored as strings parsed by `global-hotkey` (examples):
- `CmdOrCtrl+Alt+KeyC` (push)
- `CmdOrCtrl+Alt+KeyV` (pull)
- Linux default uses `Ctrl+Alt+KeyC` / `Ctrl+Alt+KeyV`.

If a hotkey fails to register (already taken or blocked), the agent will still run; you can change bindings in the config file and restart the agent.

### Restore defaults
The tray menu includes “Restore Defaults”, which resets hotkey bindings to OS-appropriate defaults while preserving connection settings (target/port/SSH options).

## Tray Menu
The tray menu includes:
- Push
- Pull
- Peek (shows metadata via notification)
- Start at login (toggle)
- Restore Defaults
- Show Config Path
- Quit

## Notifications
The agent uses OS notifications when available and falls back to stderr/logs.
- Windows: WinRT toast notifications.
- macOS: `osascript` (`display notification`) for reliable delivery from the background agent.
- Linux: `notify-rust`/DBus desktop notifications.

If hotkeys appear not to work on macOS, you may need to enable permissions for the terminal/app under:
- System Settings → Privacy & Security → Input Monitoring
- System Settings → Privacy & Security → Accessibility

If notifications do not appear on macOS:
- Check System Settings → Notifications and ensure notifications are allowed for Script Editor / `osascript`.
- Check Focus/Do Not Disturb settings.
- Inspect `logs/agent.log` for notification delivery errors.

See [Linux platform notes](#linux-platform-notes) for clipboard, hotkey and tray limits.

## Logs
When running the agent, logs are written to a `logs/agent.log` file next to the agent config file.

To increase verbosity:
- Set `RUST_LOG`, e.g. `RUST_LOG=debug`

## Start at Login

These commands affect only the retired agent. Current installed-desktop startup is in [desktop updates](../desktop-updates.md#start-at-login).

### Commands
```
ssh_clipboard autostart status
ssh_clipboard autostart enable
ssh_clipboard autostart disable
ssh_clipboard autostart refresh
```


### Scope
- Autostart is **per-user** (not system-wide).


### How it works
Autostart is implemented via `auto-launch` and points to the `ssh_clipboard_agent` binary (same folder as `ssh_clipboard`).
The autostart app name remains `ssh_clipboard` to avoid stale entries across upgrades.

`autostart enable/disable/refresh` also updates the agent config field `autostart_enabled` (used for tray state and best-effort refresh at agent startup).


### Handling moved/updated binaries
If you move or replace the binaries after enabling autostart, the stored autostart entry might point to the old path.

Recommended workflow:
- Run `ssh_clipboard autostart refresh` after moving/updating the binary.
- The agent also attempts a best-effort refresh at startup if `autostart_enabled` is set in config.
- `ssh_clipboard autostart status` and `ssh_clipboard autostart disable` can still be used even if the agent binary is currently missing.

If the current executable path cannot be resolved to a usable absolute path, autostart may fail; disable and re-enable autostart after fixing the installation location.


### Platform notes
Exact mechanisms differ per OS:
- Windows: typically a per-user startup entry.
- macOS: typically a per-user Launch Agent.
- Linux: XDG autostart entry under `~/.config/autostart/`.


## Linux Platform Notes

### Deferred Platform

The owner deferred new Linux desktop/AppImage/Wayland work on 2026-10-18. The following notes describe the frozen legacy agent, available from v0.3.0 archives or an explicit `--features agent` compatibility build. They do not describe a supported 0.4 desktop installer or an outstanding manual test assignment. See [retirement](../legacy-retirement.md).


### Prerequisites
- The Linux server exposes V2 through the legacy daemon or a shelf with an explicit V2 compatibility socket.
- The SSH user on the server can run `ssh_clipboard proxy` (binary on `PATH` or absolute path).
- The client machine has `ssh` on `PATH`.


### Clipboard Support
#### X11
Expected to work out of the box with `arboard` for text and PNG images.

#### Wayland (best effort)
- The build enables `arboard`’s `wayland-data-control` feature to use the data-control protocol when available.
- Many compositors do not support this protocol; XWayland may be required.
- In sandboxed environments (Flatpak/Snap), ensure X11/Wayland sockets are exposed.


### Agent Mode (Tray + Hotkeys)
Run from source:
```
cargo run --features agent --bin ssh_clipboard_agent
```

Recommended one-command setup:
```
ssh_clipboard setup-agent --target user@server
```

#### Hotkeys
- Global hotkeys are X11-only.
- On Wayland, the agent auto-disables hotkeys at startup and emits one notice.
- Use tray actions on Wayland, or run under X11 to enable global hotkeys.
- `--no-hotkeys` remains available as a manual override.

#### Tray
- Tray support uses GTK on Linux. Ensure GTK and an appindicator implementation are installed.
- If the tray fails to initialize, rerun with `--no-tray` and use hotkeys only.


Linux uses the XDG entry described under [Start at login](#start-at-login).

### Troubleshooting
- **Clipboard init fails:** Ensure a display server is available (`DISPLAY` for X11, `WAYLAND_DISPLAY` for Wayland).
- **Hotkeys don’t work:** On Wayland, hotkeys are auto-disabled; use tray actions or try X11.
- **Tray missing:** Install GTK + appindicator packages for your distro.


## Update Triggers
- Changes to config schema, hotkey parsing, tray menu items, or notification strategy.
- Changes to where logs are written.

## Related Docs
- [docs/cli.md](../cli.md)
- [docs/troubleshooting.md](../troubleshooting.md)
- [docs/security.md](../security.md)
