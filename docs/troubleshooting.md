# Troubleshooting

## Purpose

Locate failures in the current desktop, shelf server, shared settings and retained V2 routes without confusing their commands or limits.

## Key Files

- `src/client/diagnostics.rs`, `src/client/failure.rs`: metadata health and safe failure hints.
- `src/configuration.rs`: settings preservation and coordinated writes.
- `src/server_management.rs`: signed service activation and recovery.
- `desktop/src-tauri/src/panel.rs`, `updates.rs`: desktop operation/recovery state.

## Connection and Endpoint Selection

Use Test saved connection in desktop settings: legacy profiles diagnose V2 PeekMeta; shelf profiles negotiate sessions and request Health. The test does not fetch clipboard payloads or enable Copy by itself. Apply an edited connection before testing it.

CLI `doctor --target <alias-or-user@host>` checks the V2 route only. For shelf health, use `ssh_clipboard shelf --target <alias-or-user@host> health`. A healthy route does not establish that a separate endpoint works. See [diagnostics](connection-diagnostics.md) and the [connection matrix](compatibility-and-cutover.md#connection-matrix).

- Authentication/host-trust failure: resolve the existing key, agent or known-hosts setup in an ordinary terminal. Keep host verification enabled.
- Command unavailable: inspect the noninteractive SSH PATH or the reviewed absolute forced command. Managed installs use a per-user launcher; local service health does not prove SSH resolves it.
- A key forced to `proxy` always remains V2. A shelf profile requires a reviewed `session-proxy` route. Keep maintenance access separate; do not widen a transfer key.
- Noisy shell/MOTD: V2 permits bounded resynchronization unless `--strict-frames` is selected. Shelf framing is strict; keep its binary stdio path free of shell output.

## Managed Shelf Service

For a managed installation, inspect `systemctl --user status ssh_clipboard-shelf.service` and its user journal, then check the actual shelf SSH route. `ssh_clipboard server-maintenance status` reports saved selection/recovery metadata; it does not assert live health.

Do not start a standalone `daemon` to repair a missing shelf: that creates a separate single-item store. Socket paths must match the service and selected proxy. Run daemon/proxy as the same user; custom sockets need an owner-only parent. Investigate ownership before changing permissions or removing anything. Active sockets must be preserved.

An unfinished signed update uses explicit `server-maintenance recover --acknowledge-clipboard-loss`; review retained state and [maintenance recovery](server-maintenance.md) first. Restart and rollback lose in-memory items. Managed maintenance does not adopt the old unit or a global launcher automatically.

## Shared Settings and Drafts

- Corrupt, unsupported or unknown-field settings: preserve the original and backup before repair. The desktop disables saves over unreadable data and reloads a repaired file; it must not silently replace it with defaults.
- External revision conflict: retain any needed edits, then Discard to load the latest saved state before editing again. Transfers use saved settings while a local draft remains unapplied.
- Startup mismatch/unknown state: actual OS registration and saved preference are separate. Current source applies an explicit pending startup choice with the draft; unrelated saves leave external startup changes alone.
- Imported settings: review the backup and retained old-agent ownership. Dismissing the import review does not stop an agent or register startup.

See [shared configuration](shared-configuration.md) and [migration/recovery](configuration-migration.md). On Windows, automation-process registry reads can differ from the registry service's real-profile view; verify installation/startup in the normal desktop context. See [startup guardrails](desktop-updates.md#start-at-login).

## History, Copy and Uncertain Shares

Selection and preview do not write the local clipboard. Shelf Copy revalidates the selected item/epoch; an expired/deleted item leaves the clipboard unchanged. Legacy Copy uses the displayed snapshot, so Refresh is required to display a different server value. Direct Copy deliberately targets the newest item.

Transient shelf failures use bounded metadata reconnect; trust/authentication/protocol errors pause for explicit Reconnect history. Reconnect does not replay transfers. Cancel is not rollback: an unacknowledged Share may already have committed. Use Check share status before deciding on Retry captured share or Discard recovery data; a new clipboard sample is not a retry. See [share recovery](share-recovery.md).

## Clipboard and Limits

A Windows clipboard can be temporarily locked by another application. For Linux CLI/legacy clipboard access, a working X11/Wayland session is required; headless builds require explicit stdin/stdout/file modes. Use supported plain text or PNG; `pull --output` or `--stdout --base64` avoids writing image data to a text-only destination.

| Endpoint | Default encoded policy | Decoded-image policy |
| --- | --- | --- |
| V2 CLI/legacy profile | 10 MiB frame, including serialized metadata | 64 MiB, narrowed by an adapter's shelf policy |
| Shelf session | 64 MiB item, narrowed by negotiated client/server limits | 256 MiB default, narrowed by negotiation |

Metadata can describe a shelf item too large for a V2 Get. Raising the client limit does not change the proxy/daemon or the shelf's aggregate/resident budgets. Coordinate policies deliberately; use smaller content when appropriate. See [V2 framing](protocol.md), [shelf limits](shelf-core.md) and [client validation](shelf-client.md).

For timeouts, inspect the actual SSH failure and endpoint deadlines before increasing values. V2 uses client `--timeout-ms` and daemon/proxy `--io-timeout-ms`; shelf operation/idle/proxy policies are separate. An unacknowledged timed-out mutation remains uncertain.

## Updates, Startup and Legacy Tools

An update must pass signature and release-identity verification before Install is available. Resolve/discard an uncertain Share and apply/discard settings before installation. Verification failure does not justify bypassing trust, replacing a pinned key or rewriting a signed asset. See [desktop updates](desktop-updates.md) and [key recovery](releasing.md#key-rotation-and-recovery).

For missing native notifications, check Behavior capability/help and OS notification settings; portable Windows uses panel fallback. Shortcuts act in the background and recording suppresses transfer dispatch. See [feedback](desktop-notifications.md) and [shortcuts](desktop-shortcuts.md).

Old `install-client`, `setup-agent`, `config` and `autostart` commands require the retired agent compatibility build. Use [legacy agent guidance](legacy/agent.md), [legacy client setup](legacy/client-setup.md) and [legacy server setup](legacy/server-setup.md) only for those installations. New installs use [desktop setup](../README.md#install-the-desktop) and [managed server setup](server-setup.md).

## Update Triggers

- A diagnostic, endpoint, settings, package or recovery contract changes.
- A real failure reveals an unsafe or misleading repair instruction.

## Related Docs

- [CLI reference](cli.md), [testing](testing.md), [security](security.md).
- [Compatibility/cutover](compatibility-and-cutover.md), [docs index](index.md).
