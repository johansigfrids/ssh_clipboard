# Security

## Purpose
Describe how `ssh_clipboard` relies on SSH for transport security and provide recommended hardening for server access.

## Key Files
- `src/client/ssh.rs`
- `src/proxy.rs`
- `src/daemon.rs`
- `docs/protocol.md`

## Threat Model
In scope:
- Confidentiality and integrity of clipboard data over the network (handled by SSH).
- Preventing unintended access to the daemon socket (owner-only UNIX socket + peer credential checks).

Out of scope (for now):
- Protecting a user from their own account compromise.
- Multi-tenant sharing or fine-grained authorization beyond SSH user boundaries.

## SSH as the Security Boundary
- The client uses the system `ssh` binary and relies on standard SSH configuration:
  - host key verification (`known_hosts`)
  - key-based authentication
  - optional SSH agent usage
- The server proxy is executed over SSH and forwards requests to a local UNIX socket.

## Recommended Hardening

### 1. Per-user daemon (default)
Run the daemon as the intended user so the socket is owned by that user and has strict permissions (`0600`).

### 2. Forced command in `authorized_keys` (optional but recommended)
Restrict a dedicated key so it can only run the proxy:
```
command="ssh_clipboard proxy",no-port-forwarding,no-X11-forwarding,no-agent-forwarding,no-pty
ssh-ed25519 AAAA... your-key-comment
```

Notes:
- If `ssh_clipboard` is not on `PATH` for that user, use an absolute path in `command="..."`.
- `no-pty` helps ensure the proxy’s binary protocol isn’t corrupted by terminal behavior.

### 3. Avoid “insecure convenience” defaults
Do not disable host key checking by default. If a user wants that behavior, it should be explicit via SSH config or `--ssh-option`.

## Logging & Sensitive Data
- Avoid logging clipboard contents.
- Prefer structured logs for errors (connection failures, protocol errors, size-limit rejections).

## Implementation Notes
- Protocol version is `2` and includes `request_id` for correlating client/proxy/daemon logs.
- `--io-timeout-ms` is available on Linux `daemon`/`proxy` to avoid hung sessions.

## Update Triggers
- Adding new clipboard formats (especially binary/image) or changing the protocol.
- Changes to daemon/proxy hardening (socket permissions, peer credential checks).
- Changes to server lifecycle (autostart, systemd recommendations).

## Related Docs
- `docs/server-setup.md`
- `docs/client-setup.md`
- `docs/protocol.md`
- `ARCHITECTURE.md`
