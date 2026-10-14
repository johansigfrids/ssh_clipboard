# Managed Server Releases

## Purpose

Track Phase 7's user-owned release and recovery engine. This is a library foundation, not an exposed maintenance command or replacement for the current installer. Signature policy, bounded download, stable launcher/unit generation, systemd/proxy health adapters, and reviewed legacy migration remain to be integrated.

## Key Files

- `src/server_releases.rs`: immutable staging, owned paths/lock, atomic selection, journal, and recovery.
- `src/cli/install_daemon.rs`: existing installer; its shared global link and extracted-directory unit remain unchanged.
- [Implementation plan](implementation-plan.md), [server setup](server-setup.md), and [release operations](releasing.md).

## Current Engine

The store requires an absolute user-owned installation directory with private release directories. It copies admitted executable bytes and a bounded receipt into a same-filesystem temporary directory, flushes files/directories, then publishes an immutable version directory. Deleting the original download cannot break the retained executable. Reusing a version with different admitted bytes is refused. Paths, symlinks, ownership, permissions, byte/record limits, and a persistent exclusive lock guard writes; the engine never invokes sudo or edits global installations.

Executable selection is one relative `current -> releases/<canonical-stable-version>` link replaced atomically. Normal activation requires a newer version and reverifies both selected and previous bytes through the caller's verifier before changing selection. A durable transition journal precedes the switch. A restart must pass the caller's **protocol** health check; process/service-active status alone is insufficient. Failure restores the previous executable, restarts, and rechecks it. Initial installation failure stops the attempted service and removes selection. Failed rollback or outcome persistence retains the journal and blocks new activation until recovery.

Recovery accepts only selection matching the journal's previous/next version, conservatively restores the verified previous release, and records the outcome. It cannot restore in-memory clipboard items lost on restart. The CLI must disclose that before activation. No automatic pruning of old versions or unfinished staging directories is implemented.

## Evidence and Remaining Integration

Eight WSL filesystem/service-adapter probes pass: retained bytes after source removal, version immutability/lock exclusion, rejected admission leaving current untouched, healthy selection with retained predecessor, failed restart/protocol health, interrupted switch and subsequent recovery, failed outcome write/rollback, and unsafe/tampered paths. The full headless suite also passes. These use a deliberately fake verifier and service; they neither prove server artifact cryptography nor exercise a real systemd installation. The desktop's cryptographic tests are separate evidence.

Next integration must supply pinned-key verification with signed version/platform/channel bindings, bounded staging/download, a stable per-user launcher and actual unit file, bounded user-systemd commands, and health through the correct proxy/socket. Preserve target UID and all reviewed socket/limit options. Adoption of the old global link/service needs an explicit migration review; never replace a shared installation automatically. Validate on a disposable Linux installation before any live-host rollout. The user's current Ubuntu service has not been changed.

## Update Triggers

- Artifact/signature admission, storage layout, atomicity, or recovery behavior changes.
- CLI, service/proxy health integration, migration, or disposable/live evidence is added.

## Related Docs

- [Architecture](../ARCHITECTURE.md), [shelf server](shelf-server.md), and [testing](testing.md)
- [Server setup](server-setup.md), [releasing](releasing.md), and [docs index](index.md)
