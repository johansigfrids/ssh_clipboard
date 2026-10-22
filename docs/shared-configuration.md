# Shared Configuration

## Purpose

Desktop and CLI use one session-only settings document with explicit policy precedence.

## Key Files

- `src/configuration.rs`: profiles, limits, store and saves.
- `src/client/config.rs`: resolved session connection.
- `src/cli/settings.rs`: CLI settings and selection.

## Schema and Location

Schema 3 uses `ssh_clipboard/settings.json` under `dirs::config_dir()`. `ssh_clipboard settings path` prints the actual path. Startup reads only this file and never writes a default document. Old `settings-v1.json`, agent TOML and recovery backups are not discovered or imported. Explicit old/future schemas fail without replacing their bytes; preserve them and create fresh settings at the new path. See [cutover](compatibility-and-cutover.md).

Profiles retain stable IDs, names, target, optional port/identity, SSH options/binary, timeout and plain `limits` (`max_item_bytes`, `max_decoded_bytes`). There is no protocol selector. Preferences own shortcuts, previews, close-after-copy, desired startup, notifications and automatic update checks. Saving initializes a stable device UUID; fresh configuration intentionally creates a new identity.

Client defaults are 64 MiB encoded items, 256 MiB decoded images and 7,000 ms per operation. Session policy validates these consistently; the server may negotiate narrower limits. SSH verification/key policy remains with OpenSSH.

## CLI Selection and Edits

```text
ssh_clipboard settings set --target clipboard-hub --max-item-bytes 67108864 --max-decoded-bytes 268435456 --timeout-ms 7000
ssh_clipboard settings add-profile --id work --name Work --target work-hub
ssh_clipboard settings select-profile work
ssh_clipboard shelf health
```

`settings show` prints saved values or defaults without creating a file. `settings --settings-file <path>` and `shelf --settings-file <path>` allow explicit isolation. `settings set` supports SSH overrides, clearing port/identity/SSH options/binary, device name and preview/close preferences. Removing the selected profile selects the first remaining profile; the last profile cannot be removed.

CLI session policy precedence is explicit flag, selected profile, shared default. Direct `--target` without `--profile` uses no saved connection/policy, even if a settings file is supplied. An explicit profile permits intentional overrides. A direct Share still requires `--device-id`; profile Share uses the saved device identity unless explicitly overridden.

The CLI selects a profile before merging explicit overrides into the resolved session configuration. Explicit SSH options precede saved options, preserving OpenSSH's first-value precedence. Desktop operations resolve one configuration and use its SSH settings and policy together without caching another copy.

## Draft, Save and Reload

The desktop edits a draft; transfers and diagnostics use saved settings. Apply validates, prepares a revision-checked save under an exclusive lock, activates native settings and commits atomically. Failed activation/save preserves the previous settings and active state. Discard reloads saved values. Numeric timeout IPC stays a decimal string; limits are bounded u32 values.

Bounded reads, strict fields, stable profile identity, file snapshots, revision checks and coordinated writers prevent silent repair and lost concurrent edits. An abandoned save releases the lock. Startup preference remains separate from actual native registration. Profile, SSH or limit changes clear stale history/connections and prevent cross-profile replay; preference-only edits preserve valid session state.

## Update Triggers

- Profile/policy fields, configuration path/schema, precedence or save/reload behavior changes.

## Related Docs

- [History](desktop-history.md), [shortcuts](desktop-shortcuts.md), [CLI](cli.md), [cutover](compatibility-and-cutover.md).
