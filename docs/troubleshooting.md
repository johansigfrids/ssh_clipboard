# Troubleshooting

## Purpose
Common failure cases and how to resolve them.

## Key Files
- `docs/client-setup.md`
- `docs/server-setup.md`
- `docs/protocol.md`

## SSH Issues
- **Not sure where setup is failing:**
  - Run `ssh_clipboard doctor --target user@server` first.
  - Follow the reported failing check and hint.
- **`ssh` fails with auth/host key errors:**
  - Try `ssh -T user@server ssh_clipboard proxy` directly.
  - Fix `known_hosts` or key permissions.
- **Command not found on server:**
  - Ensure `ssh_clipboard` is on `PATH` for the SSH user.
  - Use an absolute path in `authorized_keys` forced command if needed.
- **Noisy shell / MOTD corrupts protocol:**
  - Use `ssh -T` (already default in the client) and consider forced commands.
  - Client resync is enabled by default; disable with `--strict-frames` if needed.

## Install/Uninstall Issues
- **`install-client` fails with existing files:**
  - Re-run with `--force` to overwrite existing installed binaries.
- **`install-client` succeeded but command still not found:**
  - Start a new shell/session so PATH changes are picked up.
  - Or run binaries directly from the install directory.
- **`uninstall-client` on Windows leaves `ssh_clipboard.exe`:**
  - If uninstall was launched from that same binary, deletion is deferred.
  - Close remaining processes and remove the file manually if it persists.
- **`uninstall-client` on Windows leaves `ssh_clipboard_agent.exe`:**
  - Uninstall tries to stop running agent processes, but file deletion may still be deferred if the binary is in use.
  - Close remaining processes and remove the file manually if it persists.

## Daemon/Proxy Issues
- **`daemon not running` error:**
  - Start `ssh_clipboard daemon` on the server.
  - Ensure client and proxy use the same `--socket-path`.
- **Permission denied on socket:**
  - Ensure daemon and proxy run as the same user.
  - Check directory permissions (`0700`) and socket permissions (`0600`).

## Clipboard Issues
- **Clipboard locked (Windows):**
  - Another app may be locking the clipboard. Retry.
- **Clipboard access error (macOS):**
  - Ensure the terminal/app has clipboard permissions if required by your environment.
- **Clipboard access error (Linux):**
  - Ensure a display server is available (`DISPLAY` for X11, `WAYLAND_DISPLAY` for Wayland).
  - On Wayland, clipboard support is best effort; XWayland may be required.

## Payload / Format Issues
- **`payload too large`:**
  - Default is 10 MiB. Increase `--max-size` on both client and server.
- **`unsupported content type`:**
  - Use `pull --output <file>` or `pull --stdout --base64` for non-text data.

## Timeouts
- **Operation times out:**
  - Increase `--timeout-ms` on the client.
  - Increase `--io-timeout-ms` on daemon/proxy.

## Agent Mode
- **Agent won’t start:**
  - Check `ssh_clipboard config validate` and ensure `target` is set.
  - Ensure only one instance is running (agent enforces single-instance).
- **Notifications don’t appear (macOS):**
  - The agent sends notifications via `osascript`; ensure notifications are allowed for Script Editor / `osascript` in System Settings → Notifications.
  - Check Focus/Do Not Disturb settings.
  - Check `logs/agent.log` for `notification delivery failed` messages.
- **Hotkeys don’t fire (macOS):**
  - Some environments may require enabling the terminal/app under System Settings → Privacy & Security → Input Monitoring or Accessibility.
- **Hotkeys don’t fire (Linux):**
  - Hotkeys are X11-only; on Wayland, use `--no-hotkeys`.
- **Tray icon missing (Linux):**
  - Ensure GTK + appindicator packages are installed for your distro.

## Related Docs
- `docs/client-setup.md`
- `docs/server-setup.md`
- `docs/cli.md`
- `docs/security.md`
