# Shared Configuration Foundation

## Purpose

Describe the Phase 2 configuration foundation: a shared Rust schema/store, persistent desktop settings, and CLI profile selection/edits. Legacy migration, live desktop reload, advanced profile controls, and dependency isolation remain unfinished.

## Key Files

- `src/configuration.rs`: schema, validation, bounded reads, coordinated saves.
- `src/cli/settings.rs`, `tests/shared_settings_cli.rs`: CLI edits/selection and executable-level configuration probes.
- `desktop/src-tauri/src/model.rs`: mapping the selected profile/preferences to desktop settings.
- `desktop/src-tauri/src/panel.rs`: startup restoration and shortcut/save handling.
- `desktop/src/api.ts`, `desktop/src/App.svelte`: saved state and recovery messages.

## Current Behavior

The prototype reads `ssh_clipboard/settings-v1.json` beneath the platform user configuration directory: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, and `$XDG_CONFIG_HOME` or `~/.config` on Linux. This file is separate from the old agent's configuration and from replaceable executable files. No legacy file is read, converted, or overwritten in this slice.

Opening the app reads settings only. A missing file means setup is needed; it is created when Apply succeeds. The saved document includes schema version 1, a revision, a selected profile ID, named profiles, connection target/port/key reference, and panel shortcut/preview/dismissal preferences. The current desktop edits the selected profile and preserves other profiles and their stable IDs. The CLI can add/select profiles and edit the same connection and preview/dismissal preferences. Desktop profile controls, SSH advanced options, device identity, autostart/update policy, and backup-based migration still need implementation and schema evolution.

The file contains configuration only; clipboard payloads and SSH key contents are not persisted. Starting the app does not connect, download, share, or write the clipboard. Clipboard snapshots remain process-local.

Corrupt, unsupported-version, oversized, or unrecognized-field documents produce a visible recovery message. Apply is disabled; the existing file is preserved. Repair the named file or move it aside after making a backup, then quit/reopen. Never automatically replace unreadable settings with defaults: that would lose connection choices while appearing to recover successfully.

## Save Contract and Guardrails

### CLI Selection and Edits (Rework Branch)

`ssh_clipboard settings path` prints the shared location. `settings show` prints JSON, or defaults without creating a file when absent. `settings set` updates supplied fields only; it supports `--target`, `--name`, `--port`, `--identity-file`, `--clear-port`, `--clear-identity-file`, `--show-preview true|false`, and `--close-after-copy true|false`.

```powershell
ssh_clipboard settings set --target clipboard-hub
ssh_clipboard settings add-profile --id second --name "Second server" --target other-hub
ssh_clipboard settings select-profile second
ssh_clipboard peek --profile second --ssh-option BatchMode=yes
```

Push, pull, peek, and doctor select the saved default profile when no destination is supplied. `--profile <id>` makes a one-shot selection without changing the saved default. Within a selected profile, explicit `--target`/`--host`, `--port`, and `--identity-file` override saved fields. Transport flags still override the legacy built-in defaults; advanced transport settings are not in this schema yet.

Without `--profile`, an explicit `--target` or `--host` is an independent connection: do not inherit the selected profile's port/key reference. That also provides a route around a damaged shared settings file without rewriting it. `doctor` falls back to the legacy agent config only when there is no shared document and no explicit destination/profile/file; a damaged shared document is an error, not permission to silently switch servers. Push resolves configuration before reading stdin or the OS clipboard.

All these commands accept `--settings-file <path>` for a separate file. Paths may be relative. With an explicit destination and no `--profile`, transfers bypass saved configuration even when a path was supplied. The old `config`, agent, and autostart commands still operate on the legacy agent file. Do not confuse those with the new shared `settings` namespace.

CLI settings changes currently take effect in the desktop after Quit/relaunch. A running desktop rejects a stale save rather than overwriting CLI edits; its snapshot still uses the previous active connection until restarted. Live notification/reload is pending. CLI preference edits do not register OS shortcuts or change start-at-login.

### Persistence

- Read at most 1 MiB plus one detection byte; validate schema, profile IDs/selection, and connection fields. Include the newline in the save budget.
- Hold a nonblocking OS lock on a separate `settings-v1.json.lock` file. Keep that file in place: unlinking it during replacement permits two writers to lock different files.
- Compare the current complete document against the caller's loaded snapshot under that lock. Reject stale edits instead of losing another writer's changes; revision increments on successful saves. Currently the recovery action is quit/reopen, rather than automatic reload.
- Write a unique temporary file in the destination directory, flush it with `sync_all`, then replace the destination using `NamedTempFile::persist`. An abandoned preparation leaves the previous file intact. Existing Unix directory permissions are respected; newly created directories/files are private. Windows files inherit the user's directory ACL.
- Prepare the file/check conflicts before changing OS shortcut registration. A registration rejection abandons the preparation. If replacement fails after activation, attempt to restore the previous shortcut and report any restoration failure. The footer shows the actual registered shortcut rather than claiming an inactive saved binding works.
- Preserve a saved shortcut preference on Wayland, report its unavailability, and allow other settings to be edited. Launcher/tray access remains available.

Locking is advisory and coordinates cooperating writers. Manual editors do not take this lock; concurrent manual editing is outside this contract. Atomic replacement is supported by the underlying filesystem and `tempfile`; arbitrary network filesystems and power-loss durability are not proven here. Do not claim an installer/migration backup or automatic conflict reload exists yet.

Implementation references: [Rust file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock) (requires Rust 1.89 or later) and [temporary-file replacement](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist).

## Validation

Temporary-directory Rust probes cover restoration, stale writers, competing handles, abandoned preparation, corrupt/future/unknown fields, invalid profile identity/selection, byte budgets, and failed replacement. Desktop mapping tests check profile/preference restoration and preservation of other profiles. Frontend tests check startup restoration without transfers, blocked corrupt-file saves, and failed-save drafts. Executable-level CLI tests check persisted selection, shared-store parity, relative paths, rejected/corrupt edits, and missing-profile failure before SSH/clipboard access; resolution probes cover direct destinations and explicit override precedence.

These tests never access the user's settings or OS clipboard. The user confirmed the native Windows Quit/relaunch flow on 2026-10-13: applied settings are remembered in the persistent-settings artifact. Other platforms' native persistence checks remain pending.

The 2026-10-13 target-less CLI `peek --ssh-option BatchMode=yes --json` also reached the server using the saved desktop profile and returned PNG metadata. This probe fetched no payload and changed neither clipboard.

## Update Triggers

- CLI uses this schema or live reload is added.
- Legacy configuration migration, profiles, or additional settings are implemented.
- Schema versions, file locations, save guarantees, or dependency boundaries change.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md)
- [Desktop prototype](desktop-prototype.md), [agent](agent.md), [CLI](cli.md)
- [Testing](testing.md), [dependency decisions](dependency-decisions.md)
