# SSH Clipboard Architecture

## Purpose

Describe the session-only 0.5 source: headless shared core and CLI, native desktop, and managed Linux shelf. Release identities and native observations remain in [history](docs/index.md#history-and-acceptance); source version 0.5.0 is not publication evidence.

## Key Files

- `src/client/`, `src/session_protocol/`: SSH lifecycle, negotiation and strict SCBS messages.
- `src/shelf.rs`, `src/shelf_service.rs`: bounded in-memory history, uploads and retry records.
- `src/server_socket.rs`, `src/shelf_daemon.rs`, `src/session_proxy.rs`: Linux private sockets and stdio.
- `src/server_management.rs`, `src/server_releases.rs`: signed installation and recoverable activation.
- `src/configuration.rs`, `src/cli/`: schema 3 settings and explicit session CLI.
- `desktop/src-tauri/src/clipboard_payload.rs`, `clipboard.rs`, `history.rs`, `pending.rs`: native payload ownership, previews and frozen captures.

## Current System

The Tauri/Svelte desktop and headless CLI share session transport, policy, settings, image validation and bounded file bundles. System OpenSSH connects to Linux `session-proxy`, which bridges strict SCBS stdio to a per-user shelf daemon. Current source requires negotiated session version 2 inside the unchanged version 1 SCBS framing. There is no compatibility fallback. The retired agent, V2 wire protocol, standalone daemon/proxy and old installers are removed.

The desktop owns native clipboard adapters on a retained worker thread. Only explicit Share samples the local clipboard; only explicit Copy writes it. History owns selected remote metadata and preview; pending-share state retains the frozen upload bytes, operation identity, epoch and original retry deadline. No separate latest-value cache can bypass selected-item/epoch validation.

A foreground session serves explicit operations; a separate metadata connection handles snapshots/Watch and bounded reconnect. Background refresh never fetches payloads, samples/writes the local clipboard or replays a transfer. Explicit operation results, cancellation uncertainty and recovery remain separate from connection state.

The CLI accepts explicit file/stdin input and file/stdout exports, with JSON metadata on stdout. It has no native clipboard support. Distinct persistent and one-shot session lifecycles share bounded stderr classification and safe SSH launching, including Windows console suppression, no PTY and reaping on failure.

Shared file code captures regular files/folder trees, validates a bounded manifest plus raw bytes, maps portable filenames and materializes private staging trees. The desktop uses arboard file lists and a durable bounded local cache; only Copy extracts and publishes native references. The CLI uses explicit share-files/fetch-files with permanent no-overwrite directory exports. See [file transfers](docs/file-transfers.md).

## Policy and Configuration

Schema 3 profiles in `settings.json` contain SSH settings, operation timeout and plain session limits. Core defaults are 64 MiB encoded, 256 MiB decoded and 7,000 ms. CLI precedence is explicit override, explicitly/implicitly selected profile, then defaults. A direct target without a profile is independent of saved host/key/policy. The server negotiates narrower effective limits.

Desktop drafts, guarded native activation, coordinated writers, revision checks, bounded strict reads and atomic replacement preserve settings and prevent stale writes. Profile/SSH/limit changes clear old connection/history state. Fresh settings create a device identity when saved; old files are not discovered or converted. See [configuration](docs/shared-configuration.md).

## Server and Resource Boundaries

The daemon stores explicitly shared text, PNG and file bundles in bounded memory. Count, retention, encoded storage, resident buffers, image decode and retry-record budgets remain independent. Reservations precede upload allocation; timed-out validation cannot commit later. Every upload has a valid origin and operation UUID. Retries preserve payload identity and never evict unexpired retry records to shorten a guarantee.

Socket directories are owner-only, listeners validate peer UID and exclusive ownership, and duplicate startup never unlinks a live endpoint. The default shelf path is preserved. Server payloads are never persisted; restarts create a new epoch and lose history.

Managed installation schema 2 owns a persisted daemon configuration independent of CLI parsing. Signed exact bytes, pinned trust, immutable versions, stable launcher/unit, durable journals and selected-version Health govern activation/recovery. Unsupported installations are refused before mutation; 0.5 requires [fresh cutover](docs/compatibility-and-cutover.md).

## Desktop and Distribution

The native desktop retains tray/menu, single-instance behavior, shortcuts, notifications and installed startup/update integration. Core Cargo builds are headless; the installed desktop bundles that same CLI. Resource ownership remains explicit to prevent duplicate Windows executable resources.

The existing protected desktop/server workflows remain independent. Package signing is separate from OS publisher identity. No source cleanup authorizes deployment, key changes or publication. Preserve historical archives, private rollback backups and application identity.

## Product and Privacy Contract

- Share/Copy remain explicit; selection, preview and metadata updates do not modify the local clipboard.
- Panel Copy fetches the selected item; direct Copy refreshes and fetches the newest. Expired/deleted/wrong-epoch items leave the clipboard unchanged.
- Treat content, labels and remote output as untrusted/private. Render inert previews and classify failures without exposing stderr or payloads.
- SSH aliases, keys, host verification and restricted session routes remain under OpenSSH/account policy. Maintenance access remains separate.
- Continuous synchronization, full local history, accounts/cloud relays and large streaming transfers remain outside scope.

## Update Triggers

- Protocol, dependency ownership, configuration, native lifecycle or installation boundaries change.

## Related Docs

- [Docs index](docs/index.md), [sessions](docs/persistent-sessions.md), [recovery](docs/share-recovery.md), [security](docs/security.md), [testing](docs/testing.md).
