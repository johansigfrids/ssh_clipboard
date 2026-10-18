# Docs Index

## Purpose

Maintainer and agent navigation for current behavior, development, operations, retained compatibility and dated evidence. Start with the guide that owns the affected contract; history records earlier builds and decisions.

## Key Files

- [README](../README.md): downloads and short usage/setup.
- [Architecture](../ARCHITECTURE.md): current components and system boundaries.
- [AGENTS.md](../AGENTS.md): repository working instructions.
- [Writing docs](writing-docs.md): document ownership, lifecycle and validation.

## Development

- [Desktop development](desktop-development.md): source builds, standalone artifacts, layout and native-resource guardrails.
- [Build variants](build-variants.md): default clipboard CLI, headless server/stdio, retired agent and desktop dependencies.
- [Testing](testing.md): check commands, fixture isolation, evidence boundaries and documentation checks.
- [CI](ci.md): platform matrices, prerequisites, package checks and protected release workflows.
- [Dependency decisions](dependency-decisions.md): codec, framework, configuration, adapters and signature-verifier rationale.
- [Icon assets](icon.md): shared asset generation and executable/bundle ownership.

## Desktop and Configuration

- [Shared configuration](shared-configuration.md): schema, profiles, draft/save transaction, CLI precedence and reload.
- [Configuration migration](configuration-migration.md): source selection, one-time review, exact backups and recovery.
- [History](desktop-history.md): endpoint selection, labels/privacy, selection, safe copy and mutations.
- [Shortcuts](desktop-shortcuts.md): registration, recording, background actions and keyboard behavior.
- [Feedback and notifications](desktop-notifications.md): preferences, capability fallback and retained outcomes.
- [Persistent sessions](persistent-sessions.md): SSH ownership, metadata Watch/reconnect, cancellation and no replay.
- [Share recovery](share-recovery.md): frozen captures, acknowledgment lookup and explicit protected retry/discard.
- [Desktop updates](desktop-updates.md): installed identity, startup, bounded verified downloads and installer handoff.

## Shelf and Protocol

- [Shelf core](shelf-core.md): bounded state, allocation, expiry, ordering and deduplication.
- [Session protocol](session-protocol.md): SCBS bootstrap, negotiated messages, decimal precision and payload framing.
- [Session service](shelf-service.md): stream admission, deadlines, actions and V2 adapter.
- [Linux shelf endpoint](shelf-server.md): commands, private sockets, ownership and stdio lifecycle.
- [Shelf client](shelf-client.md): session validation, explicit CLI input/export and process lifecycle.
- [V2 protocol](protocol.md): preserved SCB1 framing and latest-item compatibility contract.

## Operations and Reference

- [Managed server setup](server-setup.md): trust, user-service installation and actual SSH-route checks.
- [Server maintenance](server-maintenance.md): pinned signed releases, stable launcher/unit and rollback/recovery.
- [CLI reference](cli.md): current commands, endpoint selection and explicitly marked legacy command sections.
- [Connection diagnostics](connection-diagnostics.md): endpoint-aware metadata health and safe transport failures.
- [Troubleshooting](troubleshooting.md): desktop, settings, shelf, V2 and updater repair paths.
- [Security](security.md): SSH boundaries, private sockets and [public-key trust](security.md#signed-package-trust).
- [Production public key](update-signing-public-key.pub): public verification material; it contains no secret.
- [Releasing](releasing.md): protected signed draft staging, release notes, publication and key operations.
- [Compatibility and cutover](compatibility-and-cutover.md): protocol matrix, fixed SSH routes, migration order and return paths.
- [Released compatibility](released-compatibility.md): pinned actual v0.3.0 Linux probes and isolation.
- [Legacy retirement policy](legacy-retirement.md): current default/distribution boundary and retained 0.4 contracts.

## Legacy Compatibility

These describe explicit compatibility builds or frozen archives, not the recommended new desktop/server installation.

- [Retired agent](legacy/agent.md): configuration, tray, notifications, consolidated autostart and Linux platform notes.
- [V2 client setup](legacy/client-setup.md): older CLI/agent setup, SSH and explicit input/output.
- [Standalone V2 server setup](legacy/server-setup.md): old installer/unit/socket workflow.

## History and Acceptance

Historical pending gates and test counts apply to the source/session named in each record. They are not new manual assignments or current-head validation. Preserve package identities, dates, screenshots and unobserved limits.

- [Closed implementation plan](history/implementation-plan.md): delivered phases, compatibility commitments and deferred/non-blocking scope.
- [Completed desktop UX plan](history/desktop-ux-plan.md): six merged PRs, review criteria, screenshots and bounded validation.
- [Prototype development snapshot](history/desktop-prototype.md): original development controls, early counts and native checklist.
- [Testing checkpoints](history/testing-checkpoints.md): earlier automated/process/native results and fixture lessons.
- [Release evidence overview](history/release-readiness.md): original rework validation, draft admission and delivery closure.
- [Windows native history](history/native-history-acceptance.md): isolated migration, text/PNG, cancellation and acknowledgment recovery.
- [Windows development installer/updater](history/desktop-update-acceptance.md): actual login, native faults, key transition and cleanup.
- [0.4.0 artifact acceptance](history/release-0.4.0-acceptance.md): exact production source/packages and Windows installation correction.
- [0.4.1 desktop acceptance](history/release-0.4.1-acceptance.md): real-profile upgrade/startup and public delivery.
- [0.4.2 desktop acceptance](history/release-0.4.2-acceptance.md): immutable Windows/Apple Silicon packages and anonymous combined-feed delivery.
- [Linux cutover acceptance](history/linux-cutover-acceptance.md): signed persistent service, SSH routes and old-install retirement.
- [Mac cutover](history/macos-cutover.md): actual import, text, login/shortcut and legacy cleanup.
- [Mac updater acceptance](history/macos-updater-acceptance.md): isolated replacement/restart, settings retention and production preservation.

## Update Triggers

- A document is added, moved, merged or retired.
- Contract ownership, supported scope or lifecycle/navigation conventions change.

## Related Docs

- [Writing docs](writing-docs.md), [documentation checks](testing.md#documentation-checks).
