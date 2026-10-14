# Configuration Migration and Recovery

## Purpose

Move existing agent/prototype settings into the shared schema without losing connection choices, replacing corrupt files, or changing OS startup registrations. This is configuration migration, not installation/cutover of the old agent.

## Key Files

- `src/configuration.rs`: version detection, bounded reads, writer lock, backups, device identity.
- `src/configuration/migration.rs`: strict legacy TOML conversion and prototype schema 1 upgrade.
- `src/cli/settings.rs`: explicit migration and advanced settings edits.
- `tests/configuration_migration.rs`: temporary-file and executable migration contracts.
- `desktop/src-tauri/src/panel.rs`: startup discovery, migrated transport, import summary, and reload isolation.

## Source Selection

The stable shared path remains `ssh_clipboard/settings-v1.json` under the platform configuration directory. The historical filename is retained; the document's `schema_version`, now **2**, determines its format. Filename changes must not accidentally split desktop/CLI settings.

1. Existing shared settings take precedence. A corrupt/unsupported shared document stops initialization; never fall back to another server or overwrite it with defaults.
2. A valid prototype schema 1 document upgrades in memory on read. Reading does not rewrite it. Before the first save attempt, preserve its exact bytes at `settings-v1.schema-1.bak`, then write schema 2 through the normal coordinated/atomic store.
3. If the shared file is missing, normal desktop startup and target-less default-profile CLI discovery look for the old agent's path using `confy::get_configuration_file_path`. They read it directly, never `confy::load`, which can create defaults. Valid legacy settings initialize the default profile; a sibling `agent.pre-shared.bak` is written before the shared file becomes active. The original agent file stays intact.
4. Explicit destinations without a profile bypass saved settings and migration. An explicit `--settings-file` uses that file without default-agent discovery. Read-only `settings path`/`settings show` do not initialize missing files. Manual settings creation also takes precedence over later imports.

For an explicit migration, including an alternate legacy source:

```powershell
ssh_clipboard settings migrate-agent
ssh_clipboard settings --settings-file ./test-settings.json migrate-agent --legacy-file ./agent.toml
```

Existing shared settings make this command an idempotent no-op that prints the existing effective configuration. It does not merge another agent file over a configured profile. To import another connection today, add a profile through the settings CLI; a multi-source import UI is not implemented.

## Preserved Values and Runtime Behavior

Legacy target, port, key-file reference, SSH options, encoded-frame limit, timeout, resync choice, and resync budget are preserved and used by both desktop and CLI. CLI flags override selected-profile values even when an explicit flag equals a built-in default. CLI SSH options precede saved options, following OpenSSH's first-value rule. A direct destination without a profile remains independent of another connection's defaults.

The desktop and doctor prepend `BatchMode=yes` so a migrated `BatchMode=no` cannot trigger hidden authentication prompts. SSH's configured host-verification policy is preserved. Ordinary CLI transfers retain the selected profile's interactive behavior unless overridden.

The desktop uses the selected encoded limit for capture and full-request validation, including metadata. Its separate 64 MiB decoded-pixel safety budget remains. Increasing a client setting does not reconfigure the remote proxy/daemon; coordinated larger defaults/capability negotiation remain Phase 4 work.

Autostart choice and legacy push/pull bindings are retained in schema 2. Direct shortcuts start disabled in the new app, and import neither changes OS startup registration nor stops the old agent. Direct shortcuts now have explicit opt-in desktop controls; installed builds also expose explicit start-at-login controls with actual-registration/save recovery. Portable prototypes cannot register startup. The import summary explains the retained choices. Unknown legacy fields, unsupported versions, or invalid policy values fail rather than being silently dropped. See [cutover ownership](compatibility-and-cutover.md#configuration-and-startup) before replacing the old agent.

Schema 2 also carries a device display name, a device UUID assigned once at the first durable save/import, and a disabled-by-default future automatic-update preference. These do not enable device announcements, updater traffic, or OS startup. Basic desktop preference edits preserve all of these fields.

## Backups and Failure Recovery

- Backups preserve source bytes, including comments/formatting. Use a same-directory private temporary file, flush, and persist without overwriting an existing backup.
- A matching backup permits retry after an interrupted migration. A conflicting backup fails closed: keep both the original and prior backup, move the old backup to another safe name, then retry. Do not delete recovery copies automatically.
- Shared writers hold the normal lock during comparison, preparation, backup, and commit. A concurrent legacy-source change detected during import aborts; the original remains untouched. The old agent does not participate in the new writer lock, so import captures a verified source snapshot rather than promising a live merge.
- Corrupt/future/schema-ambiguous documents survive unchanged. Errors identify the legacy source where relevant and do not include raw TOML, key material, or clipboard contents.
- Old prototype binaries cannot read schema 2. Quit the new app and preserve its current file before manually restoring the schema 1 backup to use an old binary. Such a restore returns to that backup's settings, not subsequent edits.
- Deleting the shared file returns a running app to setup. On a later startup, an existing old agent file can be imported again. A deliberate reset of both generations requires preserving/moving both sources; deleting only the new file is not a migration-disable switch.

Automatic startup migration only writes configuration/backups; it does not install, restart a server, register startup, share a clipboard, or replay a clipboard write.

## Evidence

On 2026-10-14, temporary-file/executable probes pass for exact backup bytes, current-agent serialization, missing/invalid sources, idempotency, backup conflicts/retry, shared-file precedence, schema 1 read-only upgrade/backup, strict schema 2 fields, stable device IDs, advanced-option overrides, and preserved preferences. Desktop probes confirm migrated transport values, forced GUI batch mode, and snapshot clearing when advanced connection options change. Existing version 2 wire fixtures and PNG pixel/ownership contracts still pass.

`tests/fixtures/legacy-agent.toml` is imported under both CLI and desktop dependency lockfiles, which currently select different compatible TOML parser patch releases. Keep that fixture check when changing either graph; a compatible version range alone is not migration evidence. Local OpenSSH `-G` with an empty configuration also confirms that the prepended batch-mode value wins without connecting to a host.

A metadata-only CLI probe through the user's existing prototype profile succeeded. The settings file's SHA-256 was identical before/after, confirming that the saved file was unchanged. No legacy user file was imported by the automated probes, and no server was replaced/restarted. On 2026-10-14 the user confirmed the native Windows schema 2 Apply/Quit/relaunch check and small-image Share/Copy/paste into Paint in `SSHClipboardPrototype-20261014T064745955Z.exe`. This confirms the prototype upgrade path and native RGBA adapter; live legacy TOML import and other platforms' migration checks remain unverified.

## Update Triggers

- Configuration or legacy formats, defaults, file paths, or backup rules change.
- Start-at-login/direct shortcuts/updater become active rather than preserved preferences.
- Migration source selection or native/platform evidence changes.

## Related Docs

- [Shared configuration](shared-configuration.md), [build variants](build-variants.md)
- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md)
- [CLI](cli.md), [agent](agent.md), [testing](testing.md)
