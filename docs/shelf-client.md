# Item-Aware Shelf Client

## Purpose

Document the shared sequential client and explicit experimental CLI. The installed desktop still uses V2; the rework panel now opts into this client per profile as described in [desktop history](desktop-history.md). This client targets `ssh_clipboard session-proxy` deliberately. No service, forced command, or clipboard changes happen merely by using List/Health.

## Key Files

- `src/client/session.rs`, `src/client/session/tests.rs`: stream session, reply validation, consistent metadata snapshots, content validation.
- `src/client/session_transport.rs`: SSH stdio ownership, bounded stderr, deadline/cleanup.
- `src/client/ssh.rs`: fixed legacy/session endpoint selection.
- `src/cli/shelf.rs`: explicit input/export and mutation commands.
- `tests/shelf_cli.rs`, `tests/native_shelf_server.rs`: preflight and real disposable Linux command probes.

## Commands

The `shelf` namespace selects the new protocol. Existing `push`/`pull`/`peek` retain V2 behavior. Connection options may appear before or after the subcommand. Use a saved selected profile, `--profile <id>`, or a direct `--target <alias-or-user@host>`. Port, identity, SSH options, executable, and timeout follow the shared connection rules. Direct destinations bypass saved connection/device defaults unless a profile is explicitly selected. New payload/decode limits are independent of the saved V2 frame setting: defaults 64 MiB encoded and 256 MiB decoded, narrowed by negotiation. Framing is strict; legacy noisy-shell recovery is not silently applied to session payloads.

```text
ssh_clipboard shelf --profile home list
ssh_clipboard shelf --profile home health
ssh_clipboard shelf --profile home share --file snippet.txt
ssh_clipboard shelf --profile home share --stdin
ssh_clipboard shelf --profile home share --file screenshot.png --format png
ssh_clipboard shelf --profile home fetch <item-uuid> --stdout
ssh_clipboard shelf --profile home fetch <item-uuid> --output saved.png --epoch <server-epoch>
ssh_clipboard shelf --profile home delete <item-uuid> --epoch <server-epoch>
ssh_clipboard shelf --profile home clear --epoch <server-epoch>
```

List/Health print JSON metadata. A snapshot includes its epoch and sequence as well as items; sequence/timestamp fields retain their decimal-string wire precision. Listing never fetches payloads. Fetch requires an explicit stdout/file destination and validates the selected item's bytes before writing anything. File export stages beside the destination, syncs, and publishes without replacing an existing/competing file. A failed/missing fetch creates no destination. Stdout export contains only exact payload bytes; tracing writes to stderr, including when `RUST_LOG` enables diagnostics.

Share reads only an explicitly requested file/stdin, bounded to the encoded limit plus one overflow byte. Text must be UTF-8; PNG decode and descriptors are checked under the decoded limit. No conversion, trimming, resizing, or OS clipboard read occurs. Saved profile sharing needs the device UUID assigned by a shared settings save. Direct sharing supplies `--device-id <uuid>` and can supply `--device-name <name>`; it does not read a corrupt saved file merely to discover an identity. The server receipt time is authoritative; this CLI leaves capture time unknown. Each invocation generates one operation UUID, printed to stderr, and makes one attempt. It does not automatically retry an uncertain share or persist captured content.

Delete/Clear require the epoch returned by List. A changed epoch is rejected after Hello and before any mutation is sent; Fetch can use the same optional guard. Item UUIDs are never interpreted as list positions. The client cannot make a history snapshot transactional with a later fetch: expired/deleted items return NotFound and produce no export.

## Session and Process Contracts

`Session<T>` owns any asynchronous duplex stream and handles repeated sequential operations. Hello is bounded; the server cannot increase local item/decode limits. Validate request ID, response kind, epoch, monotonic sequence, list offset/anchor/count, selected item/representation, and returned share metadata before allocating a response body. Fetch enforces actual UTF-8/PNG bytes and PNG dimensions/native decode size, rather than trusting the descriptor. PNG validation uses the same limits-before-decoder helper as the shelf.

A complete snapshot pages under one overall deadline and rejects changing totals, duplicate IDs, or broken ordering across pages. ResyncRequired remains an explicit refresh result; no hidden retry loop combines generations. Unsupported capabilities/local invalid requests leave a healthy session usable. Partial framing, invalid replies/content, timeout, cancellation, or a server epoch change close it logically. A caller must close/drop the poisoned stream before reconnecting; it cannot reuse unread bytes. Mutation uncertainty is retained when a Share/Delete/Clear lacks a validated acknowledgment.

The CLI SSH adapter runs one bounded invocation, with the same session used for every list page. Its deadline covers connection/bootstrap, action, close, child exit, and stderr drainage together. Stderr is continuously drained with fixed chunks and bounded head/tail retention; raw remote text never enters public errors. Failed/timed-out direct children are killed/reaped with separately bounded cleanup. Cancelling the entire call drops its pipes and uses Tokio child kill-on-drop; there is no detached diagnostic reader. This is direct-child lifecycle handling, not a claim about arbitrary grandchildren.

A completed validated protocol result remains authoritative even if SSH later exits nonzero or hangs during cleanup. In particular, a confirmed commit must not become an apparently failed fresh upload. An unacknowledged mutation reports uncertainty, server epoch, and retained operation ID where applicable. Recovery that persists/reconciles a captured upload across reconnects belongs to the next transport slice.

## Evidence and Gaps

Stream probes cover 20-item paging, exact Unicode/whitespace, deduplication, selected-item fetch/delete/clear, local size refusal, wrong-item rejection before a missing body, timeout/cancellation poisoning, invalid UTF-8, mismatched PNG descriptors, raised negotiation limits, and changed epochs. Pipe-only subprocess probes cover stderr flooding, nonzero exits after valid replies, correlated errors, timeout cleanup, and missing/option targets before launch.

Native WSL probes run the real CLI through a synthetic SSH executable that execs the real session proxy against a disposable daemon. They verify exact text export, metadata-only list, no-clobber file export, missing-item cleanup, and epoch-guarded mutation with tracing enabled. Portable CLI preflight probes check explicit destinations/epochs and direct-target isolation from corrupt settings. These do not prove a new real SSH connection, desktop history controls, cross-platform clipboard ownership, persistent reconnect, or installation.

## Prevention Guidance

- Item labels are bounded content previews inside metadata. List does not fetch full payloads, but its labels can contain private text; never log them or treat them as content-free diagnostics.

- Validate response association before reserving/reading its payload, so wrong item IDs cannot make the client wait for untrusted bytes.
- Poison a stream before an asynchronous exchange; dropped futures must not leave framing reusable.
- Keep an acknowledged commit distinct from cleanup failure and an uncertain commit distinct from a rejected one.
- Stage and publish file exports without clobbering; a preflight existence check alone does not prevent races.
- Reserve stdout for JSON or explicitly selected raw bytes. All tracing belongs on stderr.

## Update Triggers

- Desktop client integration, persistent SSH ownership, recovery/subscriptions, or new representations.
- Reply validation, output/input policy, command flags, or native/SSH acceptance changes.

## Related Docs

- [Session protocol](session-protocol.md)
- [Shelf service](shelf-service.md)
- [Native endpoint](shelf-server.md)
- [Shared configuration](shared-configuration.md)
- [Implementation plan](implementation-plan.md)
