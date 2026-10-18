# Security

## Purpose
Describe how `ssh_clipboard` relies on SSH for transport security and provide recommended hardening for server access.

## Key Files
- `src/client/ssh.rs`
- `src/proxy.rs`
- `src/daemon.rs`
- [docs/protocol.md](protocol.md)

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

Current-source daemons use a stable ownership lock, preserve active sockets, reject unsafe parent/replacement paths, and verify peer UID. The shelf proxy also checks the daemon peer's UID. See [socket and process guardrails](shelf-server.md#socket-ownership-and-permissions) and the [accepted signed live deployment](history/linux-cutover-acceptance.md).

### 2. Forced command in `authorized_keys` (optional but recommended)
Restrict a dedicated key so it can only run the proxy:
```
command="ssh_clipboard proxy",no-port-forwarding,no-X11-forwarding,no-agent-forwarding,no-pty ssh-ed25519 AAAA... your-key-comment
```

Notes:
- If `ssh_clipboard` is not on `PATH` for that user, use an absolute path in `command="..."`.
- Keep the options, key type, key and comment on one authorized_keys line.
- `no-pty` helps ensure the proxy’s binary protocol isn’t corrupted by terminal behavior.
- A fixed `proxy` key remains V2. The new shelf endpoint needs a separately reviewed absolute `session-proxy` forced command and matching custom socket arguments. Use separate routes/keys for fixed legacy/new endpoints during migration. Transfer keys never gain maintenance permissions; see [compatibility and cutover](compatibility-and-cutover.md#ssh-route-and-privilege-boundary).

### 3. Avoid “insecure convenience” defaults
Do not disable host key checking by default. If a user wants that behavior, it should be explicit via SSH config or `--ssh-option`.

## Signed Package Trust

The stable production verification key is [update-signing-public-key.pub](update-signing-public-key.pub). SHA-256 of its trimmed UTF-8 value is `08c20604ca8fb74860ff1441426e3aa1d85f2ebf6acdbf44ef890d077bdcaab7` (key ID `08c20604ca8fb748`). This public file matches the independently retained owner-approved key used by the signed desktop/server builds; it contains no private signing input or SSH key.

Establish first-install trust through a reviewed checkout or independent owner confirmation of that key/fingerprint. A replacement key supplied only by a package or update feed does not establish trust. Installed desktop updates retain embedded trust; managed server installation pins the selected key. Key rotation requires the [reviewed transition/recovery procedure](releasing.md#key-rotation-and-recovery), not silently replacing this file for existing installations.

## Logging & Sensitive Data
- Avoid logging clipboard contents.
- Prefer structured logs for errors (connection failures, protocol errors, size-limit rejections).

## Implementation Notes
- V2 (`SCB1`) and shelf sessions (`SCBS`, session version 1) are separate endpoints. Both correlate requests by ID; logs must exclude clipboard payloads and content-derived labels.
- `--io-timeout-ms` is available on Linux `daemon`/`proxy` to avoid hung sessions.

## Update Triggers
- Adding new clipboard formats (especially binary/image) or changing the protocol.
- Changes to daemon/proxy hardening (socket permissions, peer credential checks).
- Changes to server lifecycle (autostart, systemd recommendations).

## Related Docs
- [docs/server-setup.md](server-setup.md)
- [docs/legacy/client-setup.md](legacy/client-setup.md)
- [docs/protocol.md](protocol.md)
- [ARCHITECTURE.md](../ARCHITECTURE.md)
