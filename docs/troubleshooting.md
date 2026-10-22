# Troubleshooting

## Purpose

Locate failures in the current desktop, shelf server, shared settings and strict session routes without confusing their commands or limits.

## Key Files

- `src/client/diagnostics.rs`, `src/client/failure.rs`: metadata health and safe failure hints.
- `src/configuration.rs`: settings preservation and coordinated writes.
- `src/server_management.rs`: signed service activation and recovery.
- `desktop/src-tauri/src/panel.rs`, `updates.rs`: desktop operation/recovery state.

## Connection and Endpoint Selection

Use Test saved connection in desktop settings to negotiate sessions and request Health. Apply edited settings before testing; Health does not fetch payloads or enable Copy.

For CLI Health, use `ssh_clipboard shelf --target <alias-or-user@host> health`. Verify the actual restricted session route.

- Authentication/host-trust failure: resolve the existing key, agent or known-hosts setup in an ordinary terminal. Keep host verification enabled.
- Command unavailable: inspect the noninteractive SSH PATH or the reviewed absolute forced command. Managed installs use a per-user launcher; local service health does not prove SSH resolves it.
- Use an explicitly reviewed `session-proxy` forced command; keep maintenance access separate and preserve key restrictions.
- Noisy shell/MOTD: session framing is strict. Keep the binary stdio path free of login output; there is no resynchronization fallback.

## Managed Shelf Service

For a managed installation, inspect `systemctl --user status ssh_clipboard-shelf.service` and its user journal, then check the actual shelf SSH route. `ssh_clipboard server-maintenance status` reports saved selection/recovery metadata; it does not assert live health.

Do not start a standalone `daemon` to repair a missing shelf: that creates a separate single-item store. Socket paths must match the service and selected proxy. Run daemon/proxy as the same user; custom sockets need an owner-only parent. Investigate ownership before changing permissions or removing anything. Active sockets must be preserved.

An unfinished signed update uses explicit `server-maintenance recover --acknowledge-clipboard-loss`; review retained state and [maintenance recovery](server-maintenance.md) first. Restart and rollback lose in-memory items. Managed maintenance does not adopt the old unit or a global launcher automatically.

## Shared Settings and Drafts

- Corrupt, unsupported or unknown-field settings: preserve the original and backup before repair. The desktop disables saves over unreadable data and reloads a repaired file; it must not silently replace it with defaults.
- External revision conflict: retain any needed edits, then Discard to load the latest saved state before editing again. Transfers use saved settings while a local draft remains unapplied.
- Startup mismatch/unknown state: actual OS registration and saved preference are separate. Current source applies an explicit pending startup choice with the draft; unrelated saves leave external startup changes alone.
- Old settings/schema: preserve the file and configure afresh at settings.json. See [fresh cutover](compatibility-and-cutover.md).

See [shared configuration](shared-configuration.md) and [fresh cutover](compatibility-and-cutover.md). On Windows, automation-process registry reads can differ from the registry service's real-profile view; verify installation/startup in the normal desktop context. See [startup guardrails](desktop-updates.md#start-at-login).

## History, Copy and Uncertain Shares

Selection and preview do not write the local clipboard. Shelf Copy revalidates the selected item/epoch; an expired/deleted item leaves the clipboard unchanged. Direct Copy deliberately targets the newest item.

Transient shelf failures use bounded metadata reconnect; trust/authentication/protocol errors pause for explicit Reconnect history. Reconnect does not replay transfers. Cancel is not rollback: an unacknowledged Share may already have committed. Use Check share status before deciding on Retry captured share or Discard recovery data; a new clipboard sample is not a retry. See [share recovery](share-recovery.md).

## Clipboard and Limits

A Windows clipboard can be temporarily locked by another application. The desktop retains native ownership; the CLI has no OS clipboard access. Use `shelf share --file/--stdin` and `shelf fetch --output/--stdout` for explicit exports.

Session defaults are 64 MiB encoded items, 256 MiB decoded images and 7,000 ms client operations; the server may narrow effective limits. Aggregate/resident budgets are separate. Inspect actual SSH failures before raising timeouts; uncertain mutations are not replayed automatically.

## Updates and Startup

An update must pass signature and release-identity verification before Install is available. Resolve/discard an uncertain Share and apply/discard settings before installation. Verification failure does not justify bypassing trust, replacing a pinned key or rewriting a signed asset. See [desktop updates](desktop-updates.md) and [key recovery](releasing.md#key-rotation-and-recovery).

If the panel still reports the previous version after replacement, compare the running app with the installed files and shortcut target. Quit the identified old process, relaunch the verified installed app through its normal shortcut and check the panel version, saved server and history. Disk version metadata alone does not prove which code an existing process has loaded. This resolved the observed Windows mismatch during the [0.5.0 migration](history/release-0.5.0-acceptance.md#completed-windows-and-mac-migration); why the old process survived replacement was not established.

For missing native notifications, check Behavior capability/help and OS notification settings; portable Windows uses panel fallback. Shortcuts act in the background and recording suppresses transfer dispatch. See [feedback](desktop-notifications.md) and [shortcuts](desktop-shortcuts.md).

Removed commands/features are not available in 0.5. Configure the desktop and managed session server using [fresh setup](compatibility-and-cutover.md).

## Update Triggers

- A diagnostic, endpoint, settings, package or recovery contract changes.
- A real failure reveals an unsafe or misleading repair instruction.

## Related Docs

- [CLI reference](cli.md), [testing](testing.md), [security](security.md).
- [Fresh setup/cutover](compatibility-and-cutover.md), [docs index](index.md).
