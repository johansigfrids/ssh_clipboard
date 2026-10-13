# Shared Configuration Foundation

## Purpose

Describe the first Phase 2 slice: a shared Rust configuration schema/store and persistent desktop settings. Legacy migration, CLI selection/overrides, extra profile controls, and dependency isolation remain unfinished.

## Key Files

- `src/configuration.rs`: schema, validation, bounded reads, coordinated saves.
- `desktop/src-tauri/src/model.rs`: mapping the selected profile/preferences to desktop settings.
- `desktop/src-tauri/src/panel.rs`: startup restoration and shortcut/save handling.
- `desktop/src/api.ts`, `desktop/src/App.svelte`: saved state and recovery messages.

## Current Behavior

The prototype reads `ssh_clipboard/settings-v1.json` beneath the platform user configuration directory: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, and `$XDG_CONFIG_HOME` or `~/.config` on Linux. This file is separate from the old agent's configuration and from replaceable executable files. No legacy file is read, converted, or overwritten in this slice.

Opening the app reads settings only. A missing file means setup is needed; it is created when Apply succeeds. The saved document includes schema version 1, a revision, a selected profile ID, named profiles, connection target/port/key reference, and panel shortcut/preview/dismissal preferences. The current desktop edits the selected profile and preserves other profiles and their stable IDs. Multiple-profile selection, SSH advanced options, device identity, autostart/update policy, CLI parity, and backup-based migration still need implementation and schema evolution.

The file contains configuration only; clipboard payloads and SSH key contents are not persisted. Starting the app does not connect, download, share, or write the clipboard. Clipboard snapshots remain process-local.

Corrupt, unsupported-version, oversized, or unrecognized-field documents produce a visible recovery message. Apply is disabled; the existing file is preserved. Repair the named file or move it aside after making a backup, then quit/reopen. Never automatically replace unreadable settings with defaults: that would lose connection choices while appearing to recover successfully.

## Save Contract and Guardrails

- Read at most 1 MiB plus one detection byte; validate schema, profile IDs/selection, and connection fields. Include the newline in the save budget.
- Hold a nonblocking OS lock on a separate `settings-v1.json.lock` file. Keep that file in place: unlinking it during replacement permits two writers to lock different files.
- Compare the current complete document against the caller's loaded snapshot under that lock. Reject stale edits instead of losing another writer's changes; revision increments on successful saves. Currently the recovery action is quit/reopen, rather than automatic reload.
- Write a unique temporary file in the destination directory, flush it with `sync_all`, then replace the destination using `NamedTempFile::persist`. An abandoned preparation leaves the previous file intact. Existing Unix directory permissions are respected; newly created directories/files are private. Windows files inherit the user's directory ACL.
- Prepare the file/check conflicts before changing OS shortcut registration. A registration rejection abandons the preparation. If replacement fails after activation, attempt to restore the previous shortcut and report any restoration failure. The footer shows the actual registered shortcut rather than claiming an inactive saved binding works.
- Preserve a saved shortcut preference on Wayland, report its unavailability, and allow other settings to be edited. Launcher/tray access remains available.

Locking is advisory and coordinates cooperating writers. Manual editors do not take this lock; concurrent manual editing is outside this contract. Atomic replacement is supported by the underlying filesystem and `tempfile`; arbitrary network filesystems and power-loss durability are not proven here. Do not claim an installer/migration backup or automatic conflict reload exists yet.

Implementation references: [Rust file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock) (requires Rust 1.89 or later) and [temporary-file replacement](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist).

## Validation

Temporary-directory Rust probes cover restoration, stale writers, competing handles, abandoned preparation, corrupt/future/unknown fields, invalid profile identity/selection, byte budgets, and failed replacement. Desktop mapping tests check profile/preference restoration and preservation of other profiles. Frontend tests check startup restoration without transfers, blocked corrupt-file saves, and failed-save drafts.

These tests never access the user's settings or OS clipboard. The native Windows prototype smoke was confirmed before persistence landed. A quit/relaunch test of the new artifact is still needed to confirm the integrated persistent-settings flow.

## Update Triggers

- CLI uses this schema or live reload is added.
- Legacy configuration migration, profiles, or additional settings are implemented.
- Schema versions, file locations, save guarantees, or dependency boundaries change.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md)
- [Desktop prototype](desktop-prototype.md), [agent](agent.md), [CLI](cli.md)
- [Testing](testing.md), [dependency decisions](dependency-decisions.md)
