# Legacy Agent Retirement

## Purpose

Record the owner's 2026-10-18 decision to retire the old tray agent and close the delivered rework without a new manual verification campaign. Distinguish obsolete installation/distribution from compatibility still used by the current CLI and desktop.

## Key Files

- `Cargo.toml`: default clipboard CLI, explicit compatibility-only `agent` feature.
- `.github/workflows/ci.yml`: default CLI contract and retained compatibility regression checks.
- `src/agent/`, `src/bin/ssh_clipboard_agent.rs`: retained opt-in source, not the supported desktop.
- `src/protocol.rs`, `src/shelf_service/legacy.rs`, `src/configuration/migration.rs`: retained V2 and import contracts.
- [Compatibility matrix](compatibility-and-cutover.md), [implementation plan](implementation-plan.md).

## Retirement Decision

The supported desktop is the signed Windows x64/Apple Silicon app. The old agent no longer belongs to the default Cargo build: `cargo build` builds the native clipboard CLI, without agent/tray/setup/autostart commands or its binary. Explicit `--features agent` remains available for compatibility through the 0.4 series. CI keeps checking that opt-in code and also checks that the ordinary CLI exposes transfers/settings without legacy agent commands.

The obsolete automatic `v*` agent/server release workflow is removed. Current releases use only the existing protected desktop/server workflows. Published v0.3.0 archives, tags and signed 0.4 assets are preserved; retiring a publisher does not rewrite historical releases. There is no scheduled new release of the old agent.

The owner's obsolete Windows/Mac agent installations and old Linux daemon/service were already removed during the accepted cutovers. This source/policy cleanup does not uninstall anything, change startup, restart the server, remove recovery backups or publish another package. It takes effect in future source builds; it does not change the released 0.4.2 binaries.

## Compatibility Kept Deliberately

- V2 `push/pull/peek`, `proxy`, legacy-server desktop profiles and the shelf's optional V2 adapter remain supported through 0.4. Current CLI routes and existing servers still use them.
- The legacy TOML reader, exact import backups and schema migration stay. Migrating old settings must not require running the retired agent.
- Retain released-client protocol fixtures, compatibility tests and opt-in agent tests as automated regression protection.
- Preserve existing published archives and private migration recovery backups. Backups are not active parallel installations.

Removing those compatibility contracts is not scheduled work for this delivery. It would require a separate explicit breaking-change decision, a route/configuration audit and updated compatibility tests. A completed host migration alone is not evidence that every user of a public release has migrated. Do not infer permission to disable a live V2 socket or rewrite a forced-command key from this retirement decision.

## Verification Scope

Use automated default CLI, opt-in agent, headless dependency, migration and V2/session tests for these changes. No manual GUI session, reboot, remote service change, publisher signing purchase or new release is needed. Existing native evidence remains valid for its recorded package/source; untested rare scenarios remain untested and non-blocking, not relabeled as passes.

On 2026-10-18, Windows locked default, opt-in agent and headless test suites and all-target warnings-as-errors lint passed, alongside formatting, default/headless dependency guards and changed-document link checks. The explicitly ignored disposable live-SSH probe was not rerun; no real clipboard or installed service was touched. CI retains the cross-platform build/regression checks.

Linux desktop/AppImage/Wayland work is deferred because the owner has no Linux desktop. Existing compilation coverage may remain as regression protection; it does not reopen native platform acceptance work.

## Update Triggers

- The owner explicitly requests compatibility removal or a new supported platform.
- Default builds, source-only agent availability or release workflows change.
- A real regression changes the need for a targeted automated or finite native check.

## Related Docs

- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md).
- [Build variants](build-variants.md), [release operations](releasing.md), [testing](testing.md).
- [Mac migration](macos-cutover.md), [live Linux cutover](linux-cutover-acceptance.md).
