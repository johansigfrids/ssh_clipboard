# Linux Shelf Endpoint

## Purpose

Document the Linux listener/stdio bridge used by the managed shelf server, its compatibility socket and bounded process lifecycle. Installation and updates are covered by [server maintenance](server-maintenance.md); exact installed-service evidence is in [Linux cutover acceptance](history/linux-cutover-acceptance.md).

## Key Files

- `src/shelf_daemon.rs`: private listener, accepted-task cap, and expiry timer.
- `src/server_socket.rs`: socket ownership, stale recovery, and safe cleanup.
- `src/session_proxy.rs`, `src/session_bridge.rs`: stdio adapter and bounded forwarding.
- `src/file_lock.rs`: explicit release of shared file-description locks.
- `src/cli/mod.rs`: Linux-only endpoint arguments.
- `tests/native_shelf_server.rs`: real daemon/proxy process probes.

## Commands and Compatibility Boundary

Build the headless Linux binary with `cargo build --no-default-features`. `ssh_clipboard shelf-daemon` runs the new in-memory shelf on `shelf.sock` in the existing per-user runtime directory. `ssh_clipboard session-proxy` is its SSH binary-stdio endpoint. Both accept an explicit `--socket-path` for a private/disposable runtime directory. The proxy does not install or autostart the daemon.

Only `shelf-daemon` and `session-proxy` serve data. The V2 listener, compatibility flags and standalone daemon/proxy commands are removed. Managed services use [installation schema 2](server-maintenance.md); older installations require [fresh setup](compatibility-and-cutover.md).

## Socket Ownership and Permissions

Create missing socket directories privately (`0700`); reject an existing final parent that is a symlink, belongs to another UID, or grants group/other permissions. Do not chmod an arbitrary existing parent to make it acceptable. A custom socket should live inside a dedicated owner-only directory.

Open a stable sibling `<socket>.lock` as a private, single-link regular file with no symlink following, then acquire its nonblocking exclusive lock. A second cooperating daemon fails before touching the socket. An existing unlocked socket is probed with a deadline: a successful/ambiguous connection is preserved; only connection-refused/missing outcomes can permit stale recovery. Check UID/file type/inode again before removal. Normal cleanup removes only the socket inode created by this binding; a replacement regular file or symlink is preserved. Keep the lock file in place to avoid locking different inodes after unlink/recreation.

The shelf listener uses this guard. The old startup path unconditionally removed the socket, which could detach a live daemon and its clipboard. It also chmodded an existing parent and temporarily changed process-wide umask. Those behaviors are replaced by private creation and explicit ownership. Native regression probes verify that duplicate startup preserves the legacy daemon's stored value.

New shelf accepts authenticate same-UID peers before dispatch, cap accepted task count before spawning, and use the service's independent admission guard. Completed tasks are collected; dropping the listener task aborts its worker set. Expiry runs every second even without incoming requests. The proxy checks the connected daemon's peer UID as well.

## Bridge and Process Lifecycle

Read a bounded Hello before connecting. A missing/untrusted daemon produces a correlated `daemon_unavailable` frame and exit 4, without reading an upload. Verify reply ID, kind, session version, and effective limits before forwarding Welcome. A framed bootstrap rejection ends the proxy. After readiness, forward both directions concurrently using one 8 KiB buffer per direction with read/write deadlines and prompt flushing. No complete clipboard payload is buffered in the proxy.

Input EOF half-closes the daemon write side and allows a bounded final response. Daemon EOF cancels the input leg. The CLI explicitly exits on both success/error: Tokio stdin uses blocking reads underneath, so dropping a pending read future alone can leave runtime shutdown waiting for SSH stdin. Tests keep the parent's owned stdin pipe open to prove that daemon EOF and missing-daemon outcomes still terminate the proxy. When a test intends to deliver pipe EOF, drop the owned pipe; `shutdown` alone is insufficient for `ChildStdin`.

These bounds cover application buffers/tasks, not all sshd/account/kernel overhead across arbitrary SSH processes.

## Executable Fixture Guardrail

Native process probes serialize test cases while preserving concurrent clients inside each case. A parallel fork can retain another test's freshly written executable descriptor until exec, causing Linux ETXTBSY after the writing parent closes it. A disposable fork/write/exec probe confirmed this. Fixture children cap Tokio workers at two. See [persistent-session evidence](persistent-sessions.md#evidence-and-fixture-guardrails); do not add upload retries to hide a fixture race.

## File Lock Release Guardrail

Linux parallel process tests exposed intermittent Busy results after completed/abandoned settings saves. A duplicated/inherited descriptor can retain an open-file-description lock after the original handle closes. `ExclusiveLock` now explicitly unlocks before closing, and settings/socket ownership use it on success and error paths. A deterministic retained-descriptor test verifies immediate release and that later closure of the old descriptor cannot release the next writer's lock. Separate persistent lock inodes, coordinated writes, and conflict detection remain required.

## Validation

The existing Ubuntu WSL environment already had Rust 1.97.1 and a C compiler; no toolchain/system packages were installed. Native Linux headless tests now run locally in `target/linux-wsl`, separate from Windows output. Unit probes cover ownership, stale recovery, files/symlinks, parent permission preservation, native listener negotiation, and the shared codec/service. Eight real-process probes cover exact text bytes through daemon/proxy, duplicate startup, fresh restart epochs, held-open-stdin cleanup, missing daemon, invalid policy before filesystem mutation, and preservation of a legacy daemon's value. Session process tests verify exact text/PNG bytes, CLI exports/mutations and restart-epoch protection.

Those WSL checks establish local process/socket behavior. Separate [real SSH probes](testing.md#transport-and-diagnostic-faults), [Windows history acceptance](history/native-history-acceptance.md), and [signed live Linux cutover](history/linux-cutover-acceptance.md) record later integration results. Automatic metadata recovery is implemented in [persistent sessions](persistent-sessions.md); signed installation/update is covered by [server maintenance](server-maintenance.md). Native Linux desktop is deferred, and actual server reboot/last-session logout remain unobserved; see the [closed scope](history/implementation-plan.md#deferred-and-non-blocking).

## Update Triggers

- Endpoint CLI/socket defaults, ownership/admission, bridge lifecycle, or compatibility changes.
- Remote SSH, installer/service migration, or supported-platform evidence lands.

## Related Docs

- [Server setup](server-setup.md)
- [Session protocol](session-protocol.md)
- [Shelf service](shelf-service.md)
- [Shared configuration](shared-configuration.md)
- [Build variants](build-variants.md)
