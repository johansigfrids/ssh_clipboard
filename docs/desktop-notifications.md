# Desktop Feedback and Notifications

## Purpose

Explain saved feedback choices, privacy, capability reporting, and retained direct-action results. These controls belong to the native desktop.

## Key Files

- `src/configuration.rs`: shared `NotificationPreference`.
- `desktop/src-tauri/src/notifications.rs`: delivery policy and native adapter.
- `desktop/src-tauri/src/panel.rs`: retained results and explicit test command.
- `desktop/src/App.svelte`: Behavior settings and result presentation.

## Preferences and Delivery

Behavior settings provide four choices for direct Share/Copy actions:

| Choice | Success | Failure |
| --- | --- | --- |
| Open panel with result (default) | Panel | Panel |
| Notify for success and errors | Native notification | Native notification |
| Notify for errors only | Quiet | Native notification |
| Keep quiet | Quiet | Quiet |

An unavailable native notification capability or a synchronous send error falls back to the panel when notification delivery was requested. Quiet actions remain quiet. If the user brings the panel forward during the operation, its inline result is sufficient. Notification failures never change a completed transfer into a transfer failure or retry it.

The latest panel or direct Share/Copy outcome is retained in memory, with an incrementing string ID so repeated identical results replace older results. It remains visible on reopening, is cleared when the active connection changes, and is discarded at Quit. It is not a persistent clipboard log. Preference-only changes retain it. Panel transfers record their success/failure after operation reconciliation and before returning or hiding the window; presentation timers never clear this backend state.

Behavior groups Startup, Clipboard panel, and Shortcut feedback as preferences for this computer across connections. Start at login appears first and retains actual-state/mismatch/unknown reporting. The feedback selector is labelled Feedback after shortcut actions. Notification help and Test notification appear only for either notification mode; unavailable capability opens the help and disables the test. A test explicitly sends synthetic content now without applying an unsaved preference. Other modes omit notification troubleshooting, and ordinary Tray menu - Available is omitted; fallback warnings remain.

Routine Apply/Discard/Test/Reset confirmations appear in the settings save area for their originating section, expire after five seconds, and clear on navigation. Late completions do not carry success into another section. Panel operation confirmations use the same bounded presentation, while the separately retained transfer outcome remains before recovery/history. Its first sentence is the summary; additional sentences remain in Details (expanded initially on failure). Single-sentence results need no duplicate disclosure.

Local errors do not expire or disappear on routine metadata/configuration updates. A successful retry of the same action or explicit Dismiss can acknowledge the local error; Dismiss never erases a retained transfer result, startup/shortcut capability state, active cancellation, or uncertain-share recovery. New errors replace older local errors. Success timers use a generation check, are cancelled when replaced/navigating/unmounting, and never own error or retained-result state.

Existing schema 2 settings without `notifications` read as `panel` without a write. New saves serialize the validated enum; unknown values are rejected and the file is preserved. Older experimental builds with strict unknown-field validation reject the added field rather than overwrite it. Use a current branch build after saving this preference.

## Privacy and Platform Capability

Native messages are fixed application-owned strings: shared, copied, or action failed/open the app. Never pass clipboard text, image labels, previews, device/SSH names, paths, raw errors, or remote response text to the native builder. Detailed errors stay in the local panel. Opening/settings edits do not request OS permission or send a notification; Test notification is a separate explicit button with synthetic content.

[Tauri documents](https://v2.tauri.app/plugin/notification/#supported-platforms) that Windows notifications require an installed app and development may use PowerShell identity. Portable/debug Windows prototypes report this capability as unavailable and use the panel fallback. Installed release builds opt into `installed-desktop`; do not turn that flag on in the portable build helper. Linux/macOS use the plugin's native adapter, pending native validation.

The locked Tauri notification plugin's desktop `permission_state`/`request_permission` always return Granted. That is not evidence of OS permission or delivery. Settings describe system settings/quiet modes and provide Test notification instead of claiming to have checked permissions. A successful submission cannot prove that the user saw a notification.

Guardrails: never mark a clipboard operation failed because its notification failed; never repeat a share to obtain a notification; never equate a plugin permission stub with native acceptance. The WebView has no notification plugin permissions; it calls only the specific Rust test command, without providing a notification body.

Prevention: one shared feedback slot previously carried Settings applied into unrelated pages and allowed routine updates to replace warnings. Keep routine success, local failure, and backend transfer retention separate. Test replacement, navigation during an in-flight action, and unmount with fake timers; shortening visible privacy copy must not weaken the native fixed-message contract above.

## Evidence

Automated policy checks cover all preference/success/failure/capability combinations. Shared store tests cover missing-field compatibility, persistence, invalid-value preservation, and unchanged read bytes. Desktop tests cover retained outcomes, IDs, profile isolation, and model mapping. Rendered tests cover draft-only edits, unsupported capability, explicit test dispatch, retained drafts, and repeated-result presentation. The [prototype snapshot](history/desktop-prototype.md#validation-status) retains early build/lint counts; [current testing](testing.md) owns check commands.

Native Windows notifications require an installed artifact and belong to the OS integration milestone. Linux/macOS delivery, quiet modes, system denial, and send failure fallback remain native checks. No notification was sent or OS preference changed by the automated suite.

## Update Triggers

- Plugin updates, stable installation identity, or installed capability changes.
- New feedback actions, privacy fields, or native delivery evidence.

## Related Docs

- [Desktop shortcuts](desktop-shortcuts.md)
- [Shared configuration](shared-configuration.md)
- [Testing](testing.md)
- [Implementation plan](history/implementation-plan.md)
