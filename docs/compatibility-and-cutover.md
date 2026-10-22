# 0.5 Fresh Setup and Cutover

## Purpose

Prepare a fresh 0.5 installation while preserving existing settings and a deliberate rollback path.

## Key Files

- `src/configuration.rs`: schema 3 shared settings.
- `src/server_management.rs`: schema 2 managed installation.
- [Archived compatibility](history/0.4/compatibility-and-cutover.md): previous contracts.

## Breaking Boundary

0.5 removes the tray agent, SCB1/V2, standalone daemon/proxy/installers, agent import and old configuration readers. Only the SCBS session protocol remains. Existing session-capable servers still speak that protocol; connecting to one does not upgrade its installation schema. Old executables and published archives remain historical artifacts.

The new desktop/CLI default is `ssh_clipboard/settings.json` under the platform configuration directory. No old shared or agent file is discovered. An explicitly supplied schema 1/2 file is refused untouched. Fresh settings get a new device identity when saved; recreate profiles and preferences intentionally.

Managed installation schema 1 is refused before store/unit/service mutations. The new installer does not adopt an existing launcher, unit or installation root.

## Fresh Client Setup

1. Preserve old settings and recovery backups; quit the previous desktop before switching installations.
2. Start the new desktop, enter the session server SSH alias, review limits and Apply. Test the saved connection.
3. Share and Copy text/PNG explicitly. Configure shortcuts, notifications, updates and startup as desired. No startup ownership is imported automatically.
4. For CLI setup use `settings set --target <alias>`; `shelf health` checks the selected profile. File/stdin/stdout transfers replace removed native clipboard commands.

## Fresh Server Setup

1. Record the current user unit, launcher target, installation root, socket policy, SSH route and independently trusted public key. Preserve exact private rollback copies outside active paths.
2. Independently verify the chosen signed package. Arrange a maintenance session separate from restricted transfer credentials.
3. Explicitly stop the owned existing service when ready; restarts lose in-memory items. Move the old installation, owned unit and launcher aside only after reviewing their identities and a concrete rollback procedure. Do not remove unrelated/global files or recovery backups.
4. Run the new verified binary's `server-maintenance install` using [server setup](server-setup.md). Do not use the old installation's ordinary update command to cross this schema break.
5. Verify selected-version local Health, then the actual noninteractive SSH `session-proxy` route. Retire any V2-only route separately; do not widen a restricted key or grant maintenance privileges.
6. Configure clients afresh and verify explicit transfers. On failure, stop the new owned service and restore the preserved old installation/unit/launcher/settings, then verify the old route. New settings are not readable by old binaries.

These are rollout instructions, not evidence of a live cutover. Source implementation and disposable tests do not authorize deployment, publication or key changes.

## Update Triggers

- Configuration schemas, SSH routing, supported commands or installation ownership change.

## Related Docs

- [Server setup](server-setup.md), [maintenance](server-maintenance.md), [shared settings](shared-configuration.md), [security](security.md).
