# Windows Native History Acceptance

## Purpose

Record the completed 2026-10-16 manual session without treating unobserved behaviors as passed. This pass is closed; remaining fault and release gates do not extend it into another series of user checkpoints.

## Key Files

- `desktop/src/App.svelte`: history, migration review, cancellation and recovery controls.
- `desktop/src-tauri/src/panel.rs`, `history.rs`, `migration_review.rs`: desktop state and explicit actions.
- `docs/testing.md`: scheduling and distinctions between automated, native and installed evidence.

## Build and Isolation

The portable Windows **SSH Clipboard History Test** used tracked source `120556c0a95c9b189d8531222e1a5d49cca75a02`. Its isolated snapshot changed only application identity/title and the compiled private configuration-store path. Synthetic legacy settings, distinct shortcuts and an owned temporary shelf were used instead of importing the owner's real configuration. A routing adapter used real system OpenSSH/authentication to the authorized Linux host and fixed private sockets. It preserved SSH host verification and used a Windows kill-on-job-close owner for its SSH child.

The shelf was a temporary process with one-hour retention and an optional V2 socket over the same store. No service installation, existing service restart, forced-command/key change, startup cutover, production signing or publication occurred. Ignored local metadata/artifacts retain fixture details; no payloads or machine-specific connection settings belong in committed evidence.

## User-Confirmed Results

| Area | Observed result |
| --- | --- |
| Import review | Review card appeared, explicit dismissal revealed the three seeded items, and dismissal remained saved after Quit/relaunch. The selected Native history test profile also survived. |
| Selected text | Selecting the older First native history item and copying replaced a deliberately different local clipboard; Notepad pasted the selected older text. |
| Native image | The blue/yellow 480 x 240 image previewed correctly and pasted into Paint with those dimensions. |
| Share and live history | Explicit Share created the expected newest item. Another client added an item without Refresh; the selected preview and local clipboard marker stayed unchanged. |
| Delete | Confirmation named the selected item; Cancel preserved it, explicit confirmation removed it, and Notepad still pasted the local marker. |
| Hidden previews | Rows used generic Text/Image labels with no content preview. Copy still worked and Notepad pasted the expected remote text. Previews could be restored. |
| Legacy profile | Switching to V2 hid history and exposed the latest item. A legacy Share appeared as newest after switching back to the shelf profile. |
| Direct Copy | With an older item selected, hidden-window direct Copy returned the newest item, reopened feedback, and pasted the expected newest text in Notepad. |
| Expiry and relaunch | The original samples expired naturally after one hour; relaunch showed the newer retained text items. Metadata inspection confirmed the same epoch. This does not prove rejection of an expired selected Copy. |
| Quit | The user saw the test tray disappear. Independent inspection after the earlier normal Quit found no test app, routing adapter or owned SSH proxy. |
| Reconnect | After an intentionally paused connection failed, Reconnect history returned to Server reachable. |
| Background cancellation | Direct Share followed by the Open shortcut exposed Cancel operation; the user clicked it. A recovery card retained the uncertain share. |
| Lost acknowledgment | A test-only relay withheld one committed Share acknowledgment. Check share status confirmed earlier acceptance and removed the recovery card. A screenshot showed the original captured text, not the replacement clipboard marker. The held connection had exited on inspection. |

The final Notepad paste of the replacement marker after Check share status was **not performed**. The native status message says the local clipboard was not read or changed, but that message is not independent paste evidence. Explicit captured retry, discard, Clear history confirmation, server-restart epoch UI and refusal of a removed/expired selected Copy were not manually exercised in this pass. Existing automated/process tests cover their contracts; do not relabel those as native results.

## Fault-Fixture Guardrail

Pausing the entire daemon before waiting for the next user action exhausted the bounded SSH connection deadline and produced a connection error instead of an active Cancel control. That first attempt is not cancellation evidence. Resume promptly and preserve the observation.

For the successful attempt, an owned test-only proxy relay stayed transparent until a Shared reply, then consumed a one-shot arm marker and withheld that acknowledgment for at most 90 seconds. Metadata remained live. The user performed Share, Open and Cancel as one timed sequence. The relay never released the deliberately lost reply; its child was bounded and cleaned up. This proves the committed-but-unacknowledged recovery path, not cancellation rollback or every pre-commit cancellation race.

Arm transfer faults when an operation actually starts, rather than before an unbounded human wait. State the expected result first and group timing-dependent actions into one checkpoint. Do not introduce another interactive micro-test when an existing automated test can verify the same contract.

## Cleanup and Remaining Gates

After the owner ended manual testing, cleanup stopped only the isolated app/routing processes and removed the owned remote directory/daemon. The controller verified that the existing legacy service PID, invocation, unit path and active state matched its baseline. The normal shared settings hash and startup registration remained unchanged. The normal agent was not running at final inspection and was restored from its existing executable with its unchanged autostart command; process presence was verified, not a new native tray observation.

This portable synthetic migration pass does not prove import of the owner's actual legacy file, an installed production build, reviewed SSH-key/service cutover, other desktop platforms, or the complete release checklist. Those remain separately scoped gates. Prefer automated fault checks next; any future user acceptance pass needs an agreed finite checklist and must state its remaining gap before it starts.

## Update Triggers

- A recorded result is contradicted or another build/platform receives native acceptance.
- A separately scoped release/migration test closes one of the named gaps.

## Related Docs

- [Desktop history](desktop-history.md), [persistent sessions](persistent-sessions.md), and [share recovery](share-recovery.md).
- [Configuration migration](configuration-migration.md), [testing](testing.md), and [implementation plan](implementation-plan.md).
