# Docs Index

## Purpose

Maintainer and agent navigation for current behavior, development, operations, fresh cutover and dated evidence. Start with the guide that owns the affected contract; history records earlier builds and decisions.

## Key Files

- [README](../README.md): downloads and short usage/setup.
- [Architecture](../ARCHITECTURE.md): current components and system boundaries.
- [AGENTS.md](../AGENTS.md): repository working instructions.
- [Writing docs](writing-docs.md): document ownership, lifecycle and validation.

## Development

- [Desktop development](desktop-development.md): source builds, standalone artifacts, layout and native-resource guardrails.
- [Build variants](build-variants.md): headless core/CLI and native desktop dependencies.
- [Testing](testing.md): check commands, fixture isolation, evidence boundaries and documentation checks.
- [CI](ci.md): platform matrices, prerequisites, package checks and protected release workflows.
- [Dependency decisions](dependency-decisions.md): codec, framework, configuration, adapters and signature-verifier rationale.
- [Icon assets](icon.md): shared asset generation and executable/bundle ownership.

## Desktop and Configuration

- [Shared configuration](shared-configuration.md): schema, profiles, draft/save transaction, CLI precedence and reload.
- [Fresh setup and cutover](compatibility-and-cutover.md): schema boundaries, backups and explicit replacement.
- [History](desktop-history.md): labels/privacy, selection, safe copy and mutations.
- [File transfers](file-transfers.md): Explorer/Finder, CLI bundles, filename handling and retained local copies.
- [Shortcuts](desktop-shortcuts.md): registration, recording, background actions and keyboard behavior.
- [Feedback and notifications](desktop-notifications.md): preferences, capability fallback and retained outcomes.
- [Persistent sessions](persistent-sessions.md): SSH ownership, metadata Watch/reconnect, cancellation and no replay.
- [Share recovery](share-recovery.md): frozen captures, acknowledgment lookup and explicit protected retry/discard.
- [Desktop updates](desktop-updates.md): installed identity, startup, bounded verified downloads and installer handoff.

## Shelf and Protocol

- [Shelf core](shelf-core.md): bounded state, allocation, expiry, ordering and deduplication.
- [Session protocol](session-protocol.md): SCBS bootstrap, negotiated messages, decimal precision and payload framing.
- [Session service](shelf-service.md): stream admission, deadlines and actions.
- [Linux shelf endpoint](shelf-server.md): commands, private sockets, ownership and stdio lifecycle.
- [Shelf client](shelf-client.md): session validation, explicit CLI input/export and process lifecycle.

## Operations and Reference

- [Managed server setup](server-setup.md): trust, user-service installation and actual SSH-route checks.
- [Server maintenance](server-maintenance.md): pinned signed releases, stable launcher/unit and rollback/recovery.
- [CLI reference](cli.md): session commands and explicit I/O.
- [Connection diagnostics](connection-diagnostics.md): session metadata health and safe transport failures.
- [Troubleshooting](troubleshooting.md): desktop, settings, shelf and updater repair paths.
- [Security](security.md): SSH boundaries, private sockets and [public-key trust](security.md#signed-package-trust).
- [Production public key](update-signing-public-key.pub): public verification material; it contains no secret.
- [Releasing](releasing.md): protected signed draft staging, release notes, publication and key operations.


## Archived 0.4 Compatibility

These guides describe removed code and past contracts, not current installation instructions.

- [Released V2 compatibility](history/0.4/released-compatibility.md), [retirement policy](history/0.4/legacy-retirement.md), [serializer decisions](history/0.4/dependency-decisions-v2.md).
- [Agent](history/0.4/legacy/agent.md), [client setup](history/0.4/legacy/client-setup.md), [standalone server](history/0.4/legacy/server-setup.md).

## History and Acceptance

Historical pending gates and test counts apply to the source/session named in each record. They are not new manual assignments or current-head validation. Preserve package identities, dates, screenshots and unobserved limits.

- [Closed implementation plan](history/implementation-plan.md): delivered phases, compatibility commitments and deferred/non-blocking scope.
- [Completed desktop UX plan](history/desktop-ux-plan.md): six merged PRs, review criteria, screenshots and bounded validation.
- [Prototype development snapshot](history/desktop-prototype.md): original development controls, early counts and native checklist.
- [Testing checkpoints](history/testing-checkpoints.md): earlier automated/process/native results and fixture lessons.
- [Release evidence overview](history/release-readiness.md): original rework validation, draft admission and delivery closure.
- [Windows native history](history/native-history-acceptance.md): isolated migration, text/PNG, cancellation and acknowledgment recovery.
- [File transfer source acceptance](history/file-transfer-source-acceptance.md): Explorer/Finder paste, native Windows/Mac clipboard round trip, cache reopen and remaining UI boundaries.
- [Windows development installer/updater](history/desktop-update-acceptance.md): actual login, native faults, key transition and cleanup.
- [0.4.0 artifact acceptance](history/release-0.4.0-acceptance.md): exact production source/packages and Windows installation correction.
- [0.4.1 desktop acceptance](history/release-0.4.1-acceptance.md): real-profile upgrade/startup and public delivery.
- [0.4.2 desktop acceptance](history/release-0.4.2-acceptance.md): immutable Windows/Apple Silicon packages and anonymous combined-feed delivery.
- [0.4.3 desktop acceptance](history/release-0.4.3-acceptance.md): signed desktop improvements, owner-confirmed Windows smoke checks and public update-feed delivery.
- [0.5.0 publication and migration acceptance](history/release-0.5.0-acceptance.md): signed packages, public delivery, fresh server/Windows/Mac cutover, native text/image checks and legacy cleanup.
- [0.6.0 file transfer release acceptance](history/release-0.6.0-acceptance.md): protected drafts, exact packages, independent verification and native evidence limits.
- [Linux cutover acceptance](history/linux-cutover-acceptance.md): signed persistent service, SSH routes and old-install retirement.
- [Mac cutover](history/macos-cutover.md): actual import, text, login/shortcut and legacy cleanup.
- [Mac updater acceptance](history/macos-updater-acceptance.md): isolated replacement/restart, settings retention and production preservation.

## Update Triggers

- A document is added, moved, merged or retired.
- Contract ownership, supported scope or lifecycle/navigation conventions change.

## Related Docs

- [Writing docs](writing-docs.md), [documentation checks](testing.md#documentation-checks).
