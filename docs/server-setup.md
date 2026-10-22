# Managed Linux Server Setup

## Purpose

Install the signed Linux x86_64 shelf server under the intended user's account, with private sockets, an explicit SSH route and a recoverable service. [Server maintenance](server-maintenance.md) owns command/storage details; [fresh cutover](compatibility-and-cutover.md) covers replacing an older installation.

## Key Files

- `src/cli/server_maintenance.rs`, `src/server_management.rs`: signed installation, unit and launcher.
- `src/server_releases.rs`, `src/server_package.rs`: immutable executables, verification and recovery.
- `src/shelf_daemon.rs`, `src/session_proxy.rs`: session route.
- [Public verification key](update-signing-public-key.pub): reviewed production trust; see [fingerprint and trust procedure](security.md#signed-package-trust).

## Prerequisites and Trust

Use a Linux x86_64 account with a functioning user systemd manager and system OpenSSH. New source uses installation schema 2 and refuses old installations; follow [fresh cutover](compatibility-and-cutover.md) before replacing one. Use the signed [server 0.5.0 package](https://github.com/johansigfrids/ssh_clipboard/releases/tag/server-v0.5.0), with [package/runtime verification and explicit native limits](history/release-0.5.0-acceptance.md). The repository-wide latest release belongs to the desktop. Do not use latest-release discovery to select server assets.

Obtain the public key through a reviewed checkout or independent owner confirmation. Download the exact requested version's binary and detached signature. Verify them without execution using an already trusted verifier:

```sh
ssh_clipboard server-maintenance verify --version <version> \
  --public-key <trusted.pub> --package <signed-binary> --signature <signed-binary.sig>
```

A key supplied only by an untrusted package/feed does not establish first-install trust. Source builds use `cargo build --release`; compiling source is separate from admitting a signed managed release.

## Install and Check

Run as the intended non-root user. Existing launcher/unit conflicts and unsafe directories are refused. Review an existing service or global link before changing it; the installer does not adopt or stop a legacy service.

```sh
ssh_clipboard server-maintenance install --version <version> \
  --public-key <trusted.pub> --package <signed-binary> --signature <signed-binary.sig> \
  --acknowledge-clipboard-loss
ssh_clipboard server-maintenance status
systemctl --user status ssh_clipboard-shelf.service
```

The installer stores immutable executables under `$XDG_DATA_HOME/ssh_clipboard/server` (normally `~/.local/share/ssh_clipboard/server`), creates `~/.local/bin/ssh_clipboard` and writes a stable user unit. No service points into a download folder. `status` reports saved selection/recovery metadata; actual service and protocol health need separate checks.

Installation accepts shelf socket/count/retention/size/deadline options; updates retain that policy. There is no V2 listener or compatibility socket. Defaults and ownership rules are in [shelf server](shelf-server.md).

Verify the noninteractive SSH command path or reviewed absolute forced command, then from a client:

```text
ssh_clipboard shelf --target <alias-or-user@host> health
ssh_clipboard shelf --target <alias-or-user@host> list
```

A restricted transfer key must allow the reviewed `session-proxy` route, with custom socket arguments matching the saved service. Keep maintenance privileges separate.

## Updates, Migration and Recovery

Use explicit versioned `server-maintenance update` and `recover` commands from [server maintenance](server-maintenance.md#managed-commands). Every mutating maintenance command acknowledges clipboard loss. Restart/rollback restores executable availability, not in-memory history. Inspect the journal and pinned trust before recovery; do not replace the key or remove records to bypass admission.

For existing installations, follow [fresh setup and cutover](compatibility-and-cutover.md) to record the old unit, binaries, routes, sockets and return path before an authorized transition. Linger or privileged legacy-link cleanup is a separate reviewed action. The completed owner [Linux cutover](history/linux-cutover-acceptance.md) records exact package/service evidence; it does not prove an actual reboot or last-session logout.

## Update Triggers

- Supported package/runtime, trust/bootstrap procedure, launcher/unit or endpoint selection changes.
- A setup failure reveals a new migration, ownership or health-check guardrail.

## Related Docs

- [Server maintenance](server-maintenance.md), [shelf endpoint](shelf-server.md), [security](security.md).
- [Troubleshooting](troubleshooting.md), [release operations](releasing.md), [legacy server setup](history/0.4/legacy/server-setup.md).
