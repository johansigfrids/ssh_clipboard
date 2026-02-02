# Agent (Hotkeys + Tray)

## Purpose
Document the background agent: tray/menu UX, global hotkeys, config, and logs.

## Key Files
- `src/agent/mod.rs`
- `src/agent/run.rs`
- `src/agent/autostart.rs`
- `src/agent/notify.rs`
- `src/main.rs`
- `docs/cli.md`

## Build and Run
The agent is enabled by default in client builds. For a Linux server-only build, use `--no-default-features`.

Run from source:
```
cargo run -- agent
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
The agent uses OS notifications when available and falls back to stderr.

If hotkeys appear not to work on macOS, you may need to enable permissions for the terminal/app under:
- System Settings → Privacy & Security → Input Monitoring
- System Settings → Privacy & Security → Accessibility

### Linux notes
- Hotkeys are X11-only; Wayland may not support global hotkeys. Use `--no-hotkeys` if registration fails.
- Tray support uses GTK; ensure a GTK/appindicator implementation is installed.
- Notifications rely on a working desktop notification daemon (DBus).

## Logs
When running the agent, logs are written to a `logs/agent.log` file next to the agent config file.

To increase verbosity:
- Set `RUST_LOG`, e.g. `RUST_LOG=debug`

## Update Triggers
- Changes to config schema, hotkey parsing, tray menu items, or notification strategy.
- Changes to where logs are written.

## Related Docs
- `docs/cli.md`
- `docs/autostart.md`
- `docs/troubleshooting.md`
- `docs/security.md`
- `docs/linux-client.md`
