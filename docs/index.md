# Docs Index

## Purpose
Internal knowledge base for maintainers and AI agents working on `ssh_clipboard`. These are not end-user docs.

## Key Files
- `docs/index.md`
- `docs/writing-docs.md`
- `AGENTS.md`
- `ARCHITECTURE.md`

## Where To Start
- Skim this index, then jump to any topic docs listed below as they are added.

## Docs Map
- `docs/writing-docs.md`: How to write and maintain internal docs in this repo.
- [ARCHITECTURE.md](../ARCHITECTURE.md): Current system and planned rework architecture.
- [docs/implementation-plan.md](implementation-plan.md): Closed rework phases, supported delivery, deferred Linux desktop and non-blocking evidence limits.
- [docs/legacy-retirement.md](legacy-retirement.md): Retired default agent/distribution, preserved 0.4 compatibility and recovery boundaries.
- [docs/desktop-prototype.md](desktop-prototype.md): Phase 1 source-build prototype, controls, limitations, and platform validation status.
- [docs/desktop-shortcuts.md](desktop-shortcuts.md): Optional direct transfers, safe recording, grouped registration, and recovery.
- [docs/desktop-notifications.md](desktop-notifications.md): Feedback preferences, private native messages, capability fallback, and retained outcomes.
- [docs/shared-configuration.md](shared-configuration.md): Shared store, desktop profiles/advanced policy, CLI overrides, live reload, and save/recovery contracts.
- [docs/configuration-migration.md](configuration-migration.md): Legacy/prototype migration, one-time import review, exact backups, schema 2 settings, preserved preferences, and recovery.
- [docs/compatibility-and-cutover.md](compatibility-and-cutover.md): Client/server protocol matrix, forced-command boundaries, upgrade order, return paths, and recorded acceptance limits.
- [docs/released-compatibility.md](released-compatibility.md): Pinned v0.3.0 Linux CLI/proxy/daemon compatibility, repeatable private process probes, and evidence boundaries.
- `docs/protocol.md`: Protocol framing, message types, and error codes.
- [docs/shelf-core.md](shelf-core.md): New bounded shelf state, upload reservations, expiry, retry contracts, and service integration.
- [docs/session-protocol.md](session-protocol.md): Separate shelf bootstrap, precision-safe metadata, negotiated limits, raw payload framing, and compatibility boundary.
- [docs/shelf-service.md](shelf-service.md): Transport-independent shelf actions, connection admission, deadlines, and stream-based lifecycle evidence.
- [docs/shelf-server.md](shelf-server.md): Linux endpoint commands, safe socket ownership, bounded stdio bridge, and native process evidence.
- [docs/shelf-client.md](shelf-client.md): Item-aware session validation, explicit CLI inputs/exports, epoch guards, and bounded SSH lifecycle.
- [docs/desktop-history.md](desktop-history.md): Explicit endpoint profiles, history selection/mutations, safe copy, private labels, and layout evidence.
- [docs/native-history-acceptance.md](native-history-acceptance.md): Closed Windows shelf/migration/cancellation session, observed native results, fixture cleanup, and explicit remaining gaps.
- [docs/persistent-sessions.md](persistent-sessions.md): Reused SSH ownership, explicit Cancel, bounded metadata Watch/recovery, and no-replay guards.
- [docs/share-recovery.md](share-recovery.md): Frozen uncertain shares, read-only acknowledgment lookup, explicit retry/discard, and server-enforced deadlines.
- [docs/desktop-updates.md](desktop-updates.md): Installed staging/startup, bounded signed updater, framework handoff, protected drafts, and native/release gates.
- [docs/server-maintenance.md](server-maintenance.md): Signed user installation/update, stable launcher/unit, bounded proxy health, crash-limit rollback/recovery, and disposable real-service evidence.
- [docs/linux-cutover-acceptance.md](linux-cutover-acceptance.md): Signed live 0.4.0 server, persistent unit/SSH health, old-installation retirement and separate rollback backup.
- [docs/macos-cutover.md](macos-cutover.md): Owner-requested complete Mac 0.3-to-0.4 replacement, package/SSH/startup gates and legacy cleanup scope.
- [docs/macos-updater-acceptance.md](macos-updater-acceptance.md): Isolated Mac update/restart rehearsal, disposable CI packages, settings separation and cleanup evidence.
- [docs/connection-diagnostics.md](connection-diagnostics.md): Desktop/CLI metadata health, safe per-step failures, one-shot SSH deadlines/drainage, and fault evidence.
- `docs/dependency-decisions.md`: Rationale and guardrails for high-impact dependency choices.
- [docs/build-variants.md](build-variants.md): Headless, clipboard-only, agent, and desktop dependency boundaries and validation evidence.
- `docs/server-setup.md`: Linux daemon setup and proxy usage.
- `docs/client-setup.md`: Windows/macOS client usage and SSH troubleshooting.
- `docs/linux-client.md`: Linux desktop client notes (Wayland/X11, tray, hotkeys).
- `docs/security.md`: Security model and recommended SSH hardening.
- [docs/update-signing-public-key.pub](update-signing-public-key.pub): Public production verification key; pin its reviewed [fingerprint and trust procedure](security.md#signed-package-trust).
- `docs/cli.md`: CLI command and flag reference.
- `docs/ci.md`: CI workflows, build variants, and release packaging.
- `docs/releasing.md`: Protected signed draft staging, verification and publication.
- [docs/release-0.4.0-acceptance.md](release-0.4.0-acceptance.md): Merged source, protected production drafts, independent signatures, Windows install/paste and isolated Linux service evidence.
- [docs/release-0.4.1-acceptance.md](release-0.4.1-acceptance.md): Signed desktop correction, real-profile upgrade/startup, publication and anonymous public-delivery verification.
- [docs/release-0.4.2-acceptance.md](release-0.4.2-acceptance.md): Windows/Apple Silicon publication, immutable package identity, anonymous combined-feed delivery and acceptance boundaries.
- [docs/release-readiness.md](release-readiness.md): Rework candidate notes, refreshed automated evidence, desktop draft admission, and remaining release gates.
- `docs/troubleshooting.md`: Common errors and fixes.
- `docs/testing.md`: How to run tests and what’s covered.
- `docs/icon.md`: Icon assets and regeneration steps.
- Retired agent compatibility references:
  - `docs/cli.md` includes `agent`, `config`, and `autostart` command notes.
  - `docs/agent.md`: Agent setup, config, hotkeys, tray UX, and logs.
  - `docs/autostart.md`: Start-at-login behavior and refresh strategy.

## Doc Conventions
- Each doc includes: `Purpose`, `Key Files`, `Update Triggers`, `Related Docs`.
- Prefer concrete paths and commands (e.g., `src/`, `cargo test`).
- Avoid secrets; reference where secrets/config are managed instead.

## Update Triggers
- Behavior changes (clipboard transfer, SSH handling, client/server workflows).
- New or changed dependencies or OS-specific steps.
- Bug fixes that clarified edge cases or error handling.

## Related Docs
- `docs/writing-docs.md`
