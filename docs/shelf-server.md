# Experimental Linux Shelf Endpoint

## Purpose

Document the new Linux listener/stdio bridge and native evidence. This branch now has runnable endpoint commands, an item-aware CLI, and desktop history source integration, but no installer/cutover or native shelf acceptance yet. The installed server has not been changed.

## Key Files

- `src/shelf_daemon.rs`: private listener, accepted-task cap, and expiry timer.
- `src/server_socket.rs`: socket ownership, stale recovery, and safe cleanup.
- `src/session_proxy.rs`, `src/session_bridge.rs`: stdio adapter and bounded forwarding.
- `src/file_lock.rs`: explicit release of shared file-description locks.
- `src/cli/mod.rs`: Linux-only endpoint arguments.
- `tests/native_shelf_server.rs`: real daemon/proxy process probes.

## Commands and Compatibility Boundary

Build the headless Linux branch with `cargo build --no-default-features`. `ssh_clipboard shelf-daemon` runs the new in-memory shelf on `shelf.sock` in the existing per-user runtime directory. `ssh_clipboard session-proxy` is its SSH binary-stdio endpoint. Both accept an explicit `--socket-path` for a private/disposable runtime directory. The proxy does not install or autostart the daemon.

The current `daemon`/`proxy` commands remain version 2 and use `daemon.sock`. To let existing clients use the new store, explicitly start `shelf-daemon --legacy-socket-path /private/runtime/daemon.sock` alongside its new `shelf.sock`. Both listeners then use one shelf, expiry timer, memory budget, and connection cap. Legacy `Set` appends an item with unknown origin; `Get`/`PeekMeta` see the latest item. Legacy clients have no history/delete controls or retry deduplication. Their frame default stays 10 MiB (`--legacy-max-frame-bytes`); PNG decode stays capped at 64 MiB, narrowed further by shelf policy. Oversized latest items return a legacy size error; metadata still reports their actual size.

Without that option, the two daemon modes have separate stores. Both sockets must be available before the new daemon starts serving: an occupied compatibility path fails startup, preserves the old endpoint, and releases the newly bound shelf socket. Stop/switch the existing service only as an explicitly authorized cutover. No forced-command key is rewritten or silently upgraded. Existing desktop/transfer commands still use version 2. New raw protocol operations are described in [the session contract](session-protocol.md); `session-proxy` is not a human text/clipboard command.

Shelf daemon options expose count/retention, encoded item/stored/resident bytes, decoded bytes, maximum connections, and operation/idle timeouts. Defaults and hard bounds are in [the shelf](shelf-core.md) and [the service](shelf-service.md). Invalid policy fails before creating the runtime directory/socket. Proxy timeouts are independent I/O safety limits, not a second payload-size policy: after verified negotiation it forwards fixed-size chunks, while daemon/client enforce negotiated frame/item bounds.

## Socket Ownership and Permissions

Create missing socket directories privately (`0700`); reject an existing final parent that is a symlink, belongs to another UID, or grants group/other permissions. Do not chmod an arbitrary existing parent to make it acceptable. A custom socket should live inside a dedicated owner-only directory.

Open a stable sibling `<socket>.lock` as a private, single-link regular file with no symlink following, then acquire its nonblocking exclusive lock. A second cooperating daemon fails before touching the socket. An existing unlocked socket is probed with a deadline: a successful/ambiguous connection is preserved; only connection-refused/missing outcomes can permit stale recovery. Check UID/file type/inode again before removal. Normal cleanup removes only the socket inode created by this binding; a replacement regular file or symlink is preserved. Keep the lock file in place to avoid locking different inodes after unlink/recreation.

Both daemon modes use this guard. The old startup path unconditionally removed the socket, which could detach a live daemon and its clipboard. It also chmodded an existing parent and temporarily changed process-wide umask. Those behaviors are replaced by private creation and explicit ownership. Native regression probes verify that duplicate startup preserves the legacy daemon's stored value.

New shelf accepts authenticate same-UID peers before dispatch, cap accepted task count before spawning, and use the service's independent admission guard. Completed tasks are collected; dropping the listener task aborts its worker set. Expiry runs every second even without incoming requests. The proxy checks the connected daemon's peer UID as well.

## Bridge and Process Lifecycle

Read a bounded Hello before connecting. A missing/untrusted daemon produces a correlated `daemon_unavailable` frame and exit 4, without reading an upload. Verify reply ID, kind, session version, and effective limits before forwarding Welcome. A framed bootstrap rejection ends the proxy. After readiness, forward both directions concurrently using one 8 KiB buffer per direction with read/write deadlines and prompt flushing. No complete clipboard payload is buffered in the proxy.

Input EOF half-closes the daemon write side and allows a bounded final response. Daemon EOF cancels the input leg. The CLI explicitly exits on both success/error: Tokio stdin uses blocking reads underneath, so dropping a pending read future alone can leave runtime shutdown waiting for SSH stdin. Tests keep the parent's owned stdin pipe open to prove that daemon EOF and missing-daemon outcomes still terminate the proxy. When a test intends to deliver pipe EOF, drop the owned pipe; `shutdown` alone is insufficient for `ChildStdin`.

These bounds cover application buffers/tasks, not all sshd/account/kernel overhead across arbitrary SSH processes. Legacy `proxy` still has its old one-shot buffering model during migration.

## Executable Fixture Guardrail

Native process probes serialize test cases while preserving concurrent clients inside each case. A parallel fork can retain another test's freshly written executable descriptor until exec, causing Linux ETXTBSY after the writing parent closes it. A disposable fork/write/exec probe confirmed this. Fixture children cap Tokio workers at two. See [persistent-session evidence](persistent-sessions.md#evidence-and-fixture-guardrails); do not add upload retries to hide a fixture race.

## File Lock Release Guardrail

Linux parallel process tests exposed intermittent Busy results after completed/abandoned settings saves. A duplicated/inherited descriptor can retain an open-file-description lock after the original handle closes. `ExclusiveLock` now explicitly unlocks before closing, and settings/socket ownership use it on success and error paths. A deterministic retained-descriptor test verifies immediate release and that later closure of the old descriptor cannot release the next writer's lock. Separate persistent lock inodes, coordinated writes, and conflict detection remain required.

## Evidence and Remaining Work

The existing Ubuntu WSL environment already had Rust 1.97.1 and a C compiler; no toolchain/system packages were installed. Native Linux headless tests now run locally in `target/linux-wsl`, separate from Windows output. Unit probes cover ownership, stale recovery, files/symlinks, parent permission preservation, native listener negotiation, and the shared codec/service. Eight real-process probes cover exact text bytes through daemon/proxy, duplicate startup, fresh restart epochs, held-open-stdin cleanup, missing daemon, invalid policy before filesystem mutation, and preservation of a legacy daemon's value. Mixed-proxy probes additionally verify legacy text into the new shelf, exact PNG return through the old proxy, clear visibility, and refusal of an occupied compatibility socket.

This is native Linux process/socket evidence in WSL, not a new remote SSH endpoint test, GUI desktop acceptance, installation, systemd migration, or live rollout. No existing Ubuntu service/clipboard was replaced/restarted. The [shared client/CLI](shelf-client.md) now has stream/process evidence. The [desktop history UI](desktop-history.md) now has automated/browser evidence. Still required: native shelf/SSH acceptance, automatic client recovery/subscriptions, signed installation/update, and supported-platform acceptance.

## Update Triggers

- Endpoint CLI/socket defaults, ownership/admission, bridge lifecycle, or compatibility changes.
- Remote SSH, installer/service migration, or supported-platform evidence lands.

## Related Docs

- [Server setup](server-setup.md)
- [Session protocol](session-protocol.md)
- [Shelf service](shelf-service.md)
- [Shared configuration](shared-configuration.md)
- [Build variants](build-variants.md)
