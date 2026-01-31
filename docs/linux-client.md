# Linux Client (Desktop)

## Purpose
Document Linux desktop client behavior, especially clipboard support, Wayland limitations, and agent (tray + hotkeys).

## Key Files
- `src/client/clipboard.rs`
- `src/agent/run.rs`
- `src/agent/notify.rs`
- `docs/agent.md`
- `docs/cli.md`

## Prerequisites
- The Linux server is running `ssh_clipboard daemon`.
- The SSH user on the server can run `ssh_clipboard proxy` (binary on `PATH` or absolute path).
- The client machine has `ssh` on `PATH`.

## Clipboard Support
### X11
Expected to work out of the box with `arboard` for text and PNG images.

### Wayland (best effort)
- The build enables `arboard`’s `wayland-data-control` feature to use the data-control protocol when available.
- Many compositors do not support this protocol; XWayland may be required.
- In sandboxed environments (Flatpak/Snap), ensure X11/Wayland sockets are exposed.

## Agent Mode (Tray + Hotkeys)
Build and run:
```
cargo run --features agent -- agent
```

### Hotkeys
- Global hotkeys are X11-only; on Wayland they may fail or be compositor-dependent.
- If hotkeys do not register, rerun with `--no-hotkeys`.

### Tray
- Tray support uses GTK on Linux. Ensure GTK and an appindicator implementation are installed.
- If the tray fails to initialize, rerun with `--no-tray` and use hotkeys only.

## Autostart
Autostart is per-user and uses XDG autostart entries.
Use:
```
ssh_clipboard autostart enable
ssh_clipboard autostart disable
ssh_clipboard autostart refresh
```

## Troubleshooting
- **Clipboard init fails:** Ensure a display server is available (`DISPLAY` for X11, `WAYLAND_DISPLAY` for Wayland).
- **Hotkeys don’t work:** On Wayland, hotkeys may not be supported; try X11 or `--no-hotkeys`.
- **Tray missing:** Install GTK + appindicator packages for your distro.

## Update Triggers
- Changes to Linux clipboard integration or Wayland support.
- Changes to Linux agent/tray/hotkey behavior.

## Related Docs
- `docs/client-setup.md`
- `docs/agent.md`
- `docs/cli.md`
- `docs/troubleshooting.md`
