# Managed Server Releases

## Purpose

Track Phase 7's signed server package policy and user-owned release/recovery engine. Only read-only package verification is exposed through the CLI; activation remains a library foundation. Bounded download, stable launcher/unit generation, systemd/proxy health adapters, and reviewed legacy migration remain to be integrated.

## Key Files

- `src/server_releases.rs`: immutable staging, owned paths/lock, atomic selection, journal, and recovery.
- `src/server_package.rs`: bounded Minisign verification, authenticated version/platform/channel, and matching Linux ELF admission.
- `tests/fixtures/server-signing.json`: public synthetic signatures and nonexecutable ELF-header fixtures.
- `src/cli/server_maintenance.rs`, `tests/server_package_cli.rs`: read-only verification command and process evidence.
- `src/cli/install_daemon.rs`: existing installer; its shared global link and extracted-directory unit remain unchanged.
- [Implementation plan](implementation-plan.md), [server setup](server-setup.md), and [release operations](releasing.md).

## Current Engine

The store requires an absolute user-owned installation directory with private release directories. It copies admitted executable bytes and a bounded receipt into a same-filesystem temporary directory, flushes files/directories, then publishes an immutable version directory. Deleting the original download cannot break the retained executable. Reusing a version with different admitted bytes is refused. Paths, symlinks, ownership, permissions, byte/record limits, and a persistent exclusive lock guard writes; the engine never invokes sudo or edits global installations.

Executable selection is one relative `current -> releases/<canonical-stable-version>` link replaced atomically. Normal activation requires a newer version and reverifies both selected and previous bytes through the caller's verifier before changing selection. A durable transition journal precedes the switch. A restart must pass the caller's **protocol** health check; process/service-active status alone is insufficient. Failure restores the previous executable, restarts, and rechecks it. Initial installation failure stops the attempted service and removes selection. Failed rollback or outcome persistence retains the journal and blocks new activation until recovery.

Recovery accepts only selection matching the journal's previous/next version, conservatively restores the verified previous release, and records the outcome. It cannot restore in-memory clipboard items lost on restart. The CLI must disclose that before activation. No automatic pruning of old versions or unfinished staging directories is implemented.

## Evidence and Remaining Integration

Eight WSL filesystem/service-adapter probes pass: retained bytes after source removal, version immutability/lock exclusion, rejected admission leaving current untouched, healthy selection with retained predecessor, failed restart/protocol health, interrupted switch and subsequent recovery, failed outcome write/rollback, and unsafe/tampered paths. The full headless suite also passes. These use a deliberately fake verifier and service; they neither prove server artifact cryptography nor exercise a real systemd installation. The desktop's cryptographic tests are separate evidence.

The concrete package policy now verifies exact bytes and the authenticated trusted-comment version/filename before admission. Names are `SSHClipboardServer-stable-linux-<arch>-<version>` for x86_64/aarch64; desktop packages or another channel/platform cannot be relabelled. Only after signature verification does a small ELF header check reject a wrong architecture/representation. Signatures/key text are bounded; packages cap at 256 MiB. The store reverifies on activation and rollback, while normal activation independently rejects version replay/downgrade. No production server signing key was created or chosen.

Three additional WSL probes use real synthetic Minisign signatures to reject tampering, wrong keys, missing/unversioned signatures, forged versions, another channel/platform, and a signed invalid ELF header. One joins this policy to staging/activation and a failed-health rollback, without executing fixture bytes. Regenerate only server fixtures with `node desktop/scripts/generate-update-test-fixtures.mjs --server`; private synthetic keys are removed and desktop fixtures remain unchanged. The shared core uses the same locked verifier version as the desktop; existing dependency pins are preserved.

Linux now exposes `ssh_clipboard server-maintenance verify --version <version> --public-key <independently-trusted.pub> --signature <package.sig> --package <package>`. It reads bounded regular files, selects the native architecture, verifies without executing bytes, and never changes a service/install. A real CLI process probe accepts the synthetic signed fixture and rejects forged versions, wrong keys, and altered bytes without changing any input file. This command is not yet a downloader, installer, or maintenance activation path.

Next integration must supply bounded downloads, pinned trust-file initialization, a stable per-user launcher and actual unit file, bounded user-systemd commands, and health through the correct proxy/socket. Preserve target UID and all reviewed socket/limit options. Adoption of the old global link/service needs an explicit migration review; never replace a shared installation automatically. Validate on a disposable Linux installation before any live-host rollout. The user's current Ubuntu service has not been changed.

## Update Triggers

- Artifact/signature admission, storage layout, atomicity, or recovery behavior changes.
- CLI, service/proxy health integration, migration, or disposable/live evidence is added.

## Related Docs

- [Architecture](../ARCHITECTURE.md), [shelf server](shelf-server.md), and [testing](testing.md)
- [Server setup](server-setup.md), [releasing](releasing.md), and [docs index](index.md)
