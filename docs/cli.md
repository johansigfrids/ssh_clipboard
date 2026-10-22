# CLI Reference

## Purpose

The CLI performs explicit session transfers and metadata operations without OS clipboard access.

## Key Files

- `src/cli/`: session commands and settings.
- `src/client/`: session/SSH transport.
- `src/server_management.rs`: managed Linux service.

## Commands

| Command | Purpose |
| --- | --- |
| `settings path/show/set` | Locate, inspect and edit shared settings. |
| `settings add-profile/select-profile/remove-profile` | Manage named connections. |
| `shelf health/list` | Metadata-only Health and consistent history snapshot, JSON on stdout. |
| `shelf share --file <path>` or `--stdin` | Share text or `--format png` bytes. |
| `shelf fetch <uuid> --output <path>` or `--stdout` | Export validated item bytes; never overwrite a file. |
| `shelf delete <uuid> --epoch <uuid>` | Delete in the observed server epoch. |
| `shelf clear --epoch <uuid>` | Clear in the observed server epoch. |
| `shelf-daemon` (Linux) | Serve bounded in-memory history on a private socket. |
| `session-proxy` (Linux) | Bridge strict SCBS stdio to the shelf socket. |
| `server-maintenance` (Linux) | Verify/install/update/recover/status for signed managed packages. |

Use `<command> --help` for complete flags. `push/pull/peek/doctor`, `daemon/proxy`, old installers, agent/config/autostart/setup commands and V2 flags are removed and fail argument parsing. There is no compatibility fallback.

## Connection and Input

`shelf` accepts `--target`, `--profile`, `--settings-file`, `--port`, `--identity-file`, repeated `--ssh-option`, `--ssh-bin`, `--timeout-ms`, `--max-item-bytes` and `--max-decoded-bytes`. Policy uses explicit flag, selected profile, then defaults. An explicit target without a profile does not inherit another connection's settings. [Shared settings](shared-configuration.md#cli-selection-and-edits) owns selection/edit rules.

Share validates identity before reading input or starting SSH, bounds input and validates UTF-8/PNG. Direct destinations need an explicit device UUID. Fetch validates complete payloads before publishing a new output file. Stdout contains JSON metadata or explicitly requested payload bytes; diagnostics go to stderr. Unacknowledged mutations are uncertain and never automatically replayed.

## Update Triggers

- Command surface, policy precedence, I/O, schema or error behavior changes.

## Related Docs

- [Shelf client](shelf-client.md), [server setup](server-setup.md), [cutover](compatibility-and-cutover.md).
