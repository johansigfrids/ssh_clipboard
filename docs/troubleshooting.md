# Troubleshooting

## Purpose
Common failure cases and how to resolve them.

## Key Files
- `docs/client-setup.md`
- `docs/server-setup.md`
- `docs/protocol.md`

## SSH Issues
- **`ssh` fails with auth/host key errors:**
  - Try `ssh -T user@server ssh_clipboard proxy` directly.
  - Fix `known_hosts` or key permissions.
- **Command not found on server:**
  - Ensure `ssh_clipboard` is on `PATH` for the SSH user.
  - Use an absolute path in `authorized_keys` forced command if needed.

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
- **Hotkeys don’t fire (macOS):**
  - Some environments may require enabling the terminal/app under System Settings → Privacy & Security → Input Monitoring or Accessibility.

## Related Docs
- `docs/client-setup.md`
- `docs/server-setup.md`
- `docs/cli.md`
- `docs/security.md`
