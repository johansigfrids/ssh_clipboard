# Shared Configuration Foundation

## Purpose

Describe shared configuration, desktop connection profiles and advanced policy, CLI parity, and live reload. [Configuration migration](configuration-migration.md) covers legacy/schema 1 conversion and backups; [build variants](build-variants.md) cover the headless boundary. OS startup/updater controls and broader platform acceptance remain pending.

## Key Files

- `src/configuration.rs`: schema, validation, bounded reads, coordinated saves.
- `src/cli/settings.rs`, `tests/shared_settings_cli.rs`: CLI edits/selection and executable-level configuration probes.
- `desktop/src-tauri/src/model.rs`: mapping the selected profile/preferences to desktop settings.
- `desktop/src-tauri/src/connection.rs`: lossless transport drafts and numeric validation at the UI boundary.
- `desktop/src-tauri/src/panel.rs`: startup restoration and shortcut/save handling.
- `desktop/src/api.ts`, `desktop/src/App.svelte`: saved state and recovery messages.

## Current Behavior

The prototype reads `ssh_clipboard/settings-v1.json` beneath the platform user configuration directory: `%APPDATA%` on Windows, `~/Library/Application Support` on macOS, and `$XDG_CONFIG_HOME` or `~/.config` on Linux. The historical filename is stable; schema 2 is now written, with a backed-up schema 1 upgrade. The file is separate from old agent configuration and replaceable executables. Startup can initialize a missing shared file from a valid, backed-up legacy agent file; it never overwrites the original. See [migration source selection](configuration-migration.md#source-selection).

Opening the app reads existing settings, or initializes from a legacy source when the shared file is missing. With neither source present it waits for setup/Apply. Schema 2 includes revisions, stable profile IDs, target/port/key reference, advanced transport settings, device identity, panel preferences, retained startup/direct-shortcut choices, and future update policy. Desktop Connection settings add/select/rename/remove profiles and edit basic/advanced policy without dropping other profiles. The CLI also edits connection/transport/preview/dismissal/device-name settings. Actual startup/updater activation and optional direct shortcuts remain later work.

The file contains configuration only; clipboard payloads and SSH key contents are not persisted. Starting the app does not connect, download, share, or write the clipboard. Clipboard snapshots remain process-local.

Corrupt, unsupported-version, oversized, or unrecognized-field documents produce a visible recovery message. Apply is disabled; the existing file is preserved. Repair the named file or move it aside after making a backup; the watcher reloads valid settings automatically. Never automatically replace unreadable settings with defaults: that would lose connection choices while appearing to recover successfully.

## Save Contract and Guardrails

### Desktop Connections

The Connection selector edits a draft. Apply selects that profile as the shared default for desktop and target-less CLI transfers; merely choosing it does not connect, fetch, or write a clipboard. Apply or Discard an edited draft before choosing another connection. The panel displays the active connection name and SSH target.

After initial setup, Add connection starts a blank connection with independent built-in transport defaults and no copied key/port/options. Apply requires a name and target and assigns a stable UUID. Discard abandons it without writing. Existing profiles retain their IDs and advanced settings when their basic fields or names are edited; renaming alone preserves the current snapshot.

Remove requires an inline confirmation and is disabled while a draft is edited or only one profile remains. Removing the active profile selects the first remaining profile in saved order. The shared removal service also supports CLI deletion of an inactive profile without switching. Removal changes local configuration only; it does not contact a server or clear its clipboard. Device identity, global preferences, and other profiles remain intact.

Both Apply and Remove take the displayed revision, compare the full saved document under the writer lock, and prepare the file before OS activation. A failed activation abandons the write; a failed replacement attempts to restore the previous shortcut. GUI saves and watcher reloads share the same activation boundary: changes to selected profile ID, destination, port, key, or transport clear the snapshot and connection status, even when two profiles use the same SSH target. This must remain true for both paths; checking only target strings during GUI Apply could otherwise expose the previous profile's item.

### Advanced Connection Policy

Connection settings have a collapsed Advanced section. Its draft belongs to the chosen profile and follows the same Apply/Discard, revision, reload, and snapshot-clearing rules as basic connection fields. Opening/typing/resetting settings never runs SSH or samples the clipboard.

| Field | Default and behavior |
| --- | --- |
| SSH options | One OpenSSH `-o` argument per line, in order. When edited, blank lines are ignored; remaining strings are preserved. The GUI still prepends `BatchMode=yes`. |
| SSH executable | Empty uses system `ssh`; a nonempty value is an executable reference/path, including spaces, without shell splitting or added arguments. |
| Timeout | 7,000 milliseconds. |
| Transfer limit | 10,485,760 encoded bytes including metadata. This client policy does not reconfigure the remote proxy/daemon; image decoding retains its separate 64 MiB safety limit. |
| Framing recovery | Enabled, bounded by 8,192 bytes; disabling selects strict framing and retains the saved budget. |

Restore advanced defaults only edits the current draft; Discard can undo it before Apply. New profiles start with independent defaults. Existing/migrated values load directly, including values from CLI edits.

`TransportDraft` carries numeric values as decimal **strings** across IPC, then parses positive integers into Rust's actual field types and calls the shared policy validator. The schema 2 file still uses JSON numbers. Never convert these IPC fields through JavaScript `Number`: an unrelated preference edit could otherwise round a large saved timeout/byte count. Deep-copy nested transport/options when creating a draft or choosing a profile, or bindings could mutate the active view and make dirty detection fail. Keep raw multiline editor text separate from parsed options so a trailing newline can be typed while blank lines stay out of submitted arguments.

### CLI Selection and Edits (Rework Branch)

`ssh_clipboard settings path` prints the shared location. `settings show` prints effective JSON, or defaults without creating a file when absent. Schema 1 reads show their upgraded in-memory representation. `settings set` updates supplied fields only; see [CLI](cli.md) for basic/advanced flags. `settings migrate-agent` explicitly initializes from a backed-up legacy source only when shared settings do not already exist.

```powershell
ssh_clipboard settings set --target clipboard-hub
ssh_clipboard settings add-profile --id second --name "Second server" --target other-hub
ssh_clipboard settings select-profile second
ssh_clipboard peek --profile second --ssh-option BatchMode=yes
ssh_clipboard settings remove-profile second
```

Push, pull, peek, and doctor select the saved default profile when no destination is supplied. `--profile <id>` makes a one-shot selection without changing the saved default. Within a selected profile, explicit target/host/port/key/binary/size/timeout/resync flags override saved fields, then built-in defaults. CLI SSH options precede saved options. `--resync-frames true|false` explicitly overrides a saved choice; `--strict-frames` forces strict mode. Doctor supports its existing subset of overrides and uses the selected policy for metadata probes.

Without `--profile`, an explicit `--target` or `--host` is an independent connection: do not inherit the selected profile's port/key/options. This also bypasses a damaged shared settings file without rewriting it. Default discovery can migrate a valid legacy file if shared settings are absent; a damaged shared document is an error, not permission to switch servers. Doctor uses the same discovery rather than the old `confy::load` fallback, which could create an empty old-agent file. Push resolves configuration before reading stdin or the OS clipboard.

All these commands accept `--settings-file <path>` for a separate file. Paths may be relative. `remove-profile <id>` rejects missing/last profiles without writing; deleting the default selects the first remaining profile. With an explicit destination and no `--profile`, transfers bypass saved configuration even when a path was supplied. The old `config`, agent, and autostart commands still operate on the legacy agent file. Do not confuse those with the new shared `settings` namespace.

The desktop checks the shared file every two seconds and notifies the frontend only when settings or their recovery state change. It defers activation while a transfer/settings operation holds the operation lock. Valid CLI edits take effect without restart. Connection or selected-profile changes clear the old snapshot and connection status, including when two profiles have the same target. Preference-only changes preserve the snapshot. Reload never connects, fetches content, reads the OS clipboard, or replays a write.

An edited frontend draft is preserved when external settings arrive, but Apply is blocked until the user discards the stale draft. Drafts and transfer/test actions carry the displayed settings revision as a string; the backend rejects an obsolete revision before any SSH/clipboard access. This closes the race between backend activation and frontend notification. A rejected action refreshes the panel state without retrying the transfer. Unchanged files do not repeatedly register shortcuts or generate preview work.

If reload encounters a damaged file or rejected OS shortcut, previous active settings remain usable and a recovery notice blocks saves over the file. File repair is retried automatically; repeated identical failures generate one frontend notice. CLI preference edits themselves do not register shortcuts or change start-at-login. The desktop applies supported OS changes during activation. Changing advanced connection policy also clears the previous snapshot; device/display-only changes retain it.

The saved connection's diagnostic report follows the same profile/policy isolation as its snapshot. It is hidden while the UI draft is edited, retained for display-only/preference changes, and cleared by a connection change. Test uses the shared metadata-only service, never a clipboard read/write or general remote shell probe; see [connection diagnostics](connection-diagnostics.md).

### Persistence

- Read at most 1 MiB plus one detection byte; validate schema, profile IDs/selection, and connection fields. Include the newline in the save budget.
- Hold a nonblocking OS lock on a separate `settings-v1.json.lock` file. Keep that file in place: unlinking it during replacement permits two writers to lock different files.
- Compare the current complete document against the caller's loaded snapshot under that lock. Reject stale edits instead of losing another writer's changes; revision increments on successful saves. The desktop watcher reloads external edits, while its separate draft revision prevents silently saving an old draft over the new active snapshot.
- Write a unique temporary file in the destination directory, flush it with `sync_all`, then replace the destination using `NamedTempFile::persist`. An abandoned preparation leaves the previous file intact. Existing Unix directory permissions are respected; newly created directories/files are private. Windows files inherit the user's directory ACL.
- Prepare the file/check conflicts before changing OS shortcut registration. A registration rejection abandons the preparation. If replacement fails after activation, attempt to restore the previous shortcut and report any restoration failure. The footer shows the actual registered shortcut rather than claiming an inactive saved binding works.
- Preserve a saved shortcut preference on Wayland, report its unavailability, and allow other settings to be edited. Launcher/tray access remains available.

Locking is advisory and coordinates cooperating writers. Manual editors do not take this lock; concurrent manual editing is outside this contract. Atomic replacement is supported by the underlying filesystem and `tempfile`; arbitrary network filesystems and power-loss durability are not proven here. Migration backups exist; installer/OS rollback remains separate future work. Polling can observe a manually truncated file as invalid before a later valid replacement; preserve the last active state while reporting recovery.

Implementation references: [Rust file locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock) (requires Rust 1.89 or later) and [temporary-file replacement](https://docs.rs/tempfile/latest/tempfile/struct.NamedTempFile.html#method.persist).

## Validation

Temporary-directory Rust probes cover restoration, stale writers, competing handles, abandoned preparation, corrupt/future/unknown fields, invalid profile identity/selection, byte budgets, and failed replacement. Desktop mapping tests check profile/preference restoration and preservation of other profiles. Frontend tests check startup restoration without transfers, blocked corrupt-file saves, and failed-save drafts. Executable-level CLI tests check persisted selection, shared-store parity, relative paths, rejected/corrupt edits, and missing-profile failure before SSH/clipboard access; resolution probes cover direct destinations and explicit override precedence.

These tests never access the user's settings or OS clipboard. The user confirmed the native Windows Quit/relaunch flow on 2026-10-13: applied settings are remembered in the persistent-settings artifact. Other platforms' native persistence checks remain pending.

Live-reload backend probes cover profile isolation, preference-only retention, unchanged-file silence, corrupted-file recovery, rejected OS activation, and deletion back to setup. Frontend probes cover preserving/invalidating drafts, clean-form reload, notifications during startup/a pending transfer, subscription-failure cleanup, and stale transfer guards. Subscribe before the initial state read, queue startup notifications until it completes, and keep controls disabled until ready; otherwise an edit can be missed or an early draft replaced by initialization. Clean up successful subscriptions if another registration fails. On 2026-10-14 the user confirmed the native Windows app/CLI live-reload check: changing preview visibility through CLI updates the open app, and restoring it works. Fault probes still use controlled activation callbacks.

The 2026-10-13 target-less CLI `peek --ssh-option BatchMode=yes --json` also reached the server using the saved desktop profile and returned PNG metadata. This probe fetched no payload and changed neither clipboard.

On 2026-10-14, temporary-store desktop probes cover creating/renaming/removing connections, UUID stability, preserved device/preferences/transport, same-target profile isolation, stale saves, activation rejection, and replacement rollback. Executable CLI checks cover deleting selected/inactive profiles and refusing missing/last profiles without changing bytes. Rendered frontend checks cover draft selection, cancelling creation, confirmation/cancellation, failed removal, and recovery after external deletion. Separately, on 2026-10-14 the user confirmed native Windows creation/Test, switching with preview clearing, rename/relaunch persistence, and removal back to the original connection in SSHClipboardPrototype-20261014T070735201Z.exe.

Advanced-policy probes cover exact integers beyond JavaScript's safe range, rejected zero/negative/fractional/overflowing values and embedded option control characters, per-profile persistence and actual client policy, retained batch mode, isolated nested drafts, multiline option editing, reset/Discard, strict-budget retention, and stale-draft recovery. These are temporary-store/rendered UI checks; native advanced-control persistence/Test remains pending.

## Update Triggers

- CLI uses this schema or live reload is added.
- Legacy configuration migration, profiles, or additional settings are implemented.
- Schema versions, file locations, save guarantees, or dependency boundaries change.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md)
- [Desktop prototype](desktop-prototype.md), [agent](agent.md), [CLI](cli.md)
- [Testing](testing.md), [dependency decisions](dependency-decisions.md)
