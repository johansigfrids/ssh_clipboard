# Desktop Shortcuts

## Purpose

Describe optional direct transfers, recording, and OS registration recovery in the rework desktop. These controls do not change the legacy agent's registrations.

## Key Files

- `desktop/src-tauri/src/shortcuts.rs`: parsed binding set and transactional OS changes.
- `desktop/src-tauri/src/panel.rs`: dispatch, operation reservation, and transfer helpers.
- `desktop/src/App.svelte`, `desktop/src/api.ts`, `desktop/src/shortcut.js`: settings drafts, recording, and feedback.

## Behavior

Open panel defaults to `CommandOrControl+Shift+V`. Optional Share current clipboard and Copy shared clipboard have no default bindings and remain disabled until enabled and applied. Imported bindings remain visible but inactive until explicitly enabled. Either direct binding can be cleared independently; disabling retains both choices.

Direct actions work while the window is in the background. Share captures once and sends once, without automatic retry. Copy fetches the latest remote item, validates it, then writes it to the retained native clipboard owner. Empty/failed fetches leave the local clipboard untouched. The panel Copy button continues to copy its displayed snapshot. Results open the panel by default; [feedback preferences](desktop-notifications.md) can select private native notifications or quiet behavior, with the latest result retained in the panel.

Explain the complete interaction when configuring or testing these bindings: copy normally, press Share to send the existing local clipboard; press Copy to receive the remote item, then paste normally. Neither shortcut copies a selection or pastes into the active app. A manual test must state its expected visible result, including the feedback preference in effect.

While the window has focus, registered key presses feed the recorder or show an explanation, without transferring. This also supports recording an already registered binding even when the OS consumes its browser key event. The recorder first acknowledges backend suppression; cancellation/completion clear it in order. Escape or focus loss cancels recording. A failed reset leaves transfers suppressed until recovery/relaunch. Repeated Pressed events are suppressed until Released. Native focus-query failure suppresses direct actions. The callback reserves the session immediately, so busy presses are ignored instead of queuing captures that could sample later content or another profile. Opening, recording, saving, and reloading never sample or transfer content.

## Registration and Recovery

Parse all enabled bindings before changing the OS. Compare canonical key IDs so aliases cannot assign one key to two actions. Register additions before releasing old bindings; swaps between actions reuse registrations. Keep the old dispatch map until the whole change succeeds. On failure, reconcile the OS set to the previous one and report any restoration failure. Track leftover registrations, but give newly stranded keys no action. A failed file replacement attempts to restore old bindings; if that restoration fails, suspend direct dispatch until recovery/relaunch.

Guardrail: treating shortcuts as independent strings can miss aliases, fail swaps, or activate a partially applied Share binding. Keep OS inventory separate from saved preferences and never dispatch from the requested draft. Failed cleanup must not be hidden by a successful-looking status or forgotten registration.

Wayland preserves preferences and reports launcher/tray fallback; the X11 plugin is not initialized there. GlobalShortcuts portal support remains pending.

## Evidence and Remaining Checks

Automated checks cover alias conflicts, inactive retained bindings, action swaps without re-registration, partial registration failure, old-key removal failure, stranded-key suppression, and settings persistence. Rendered tests cover independent recorders, native-consumed keys, cancellation/repeats, retained disabled choices, success feedback, and absence of transfer commands while editing. They do not prove native focus, OS conflict handling, or clipboard behavior from a global key press.

On 2026-10-14 the user confirmed background direct Share on Windows with `SSHClipboardPrototype-20261014T112019571Z.exe`: copy text normally, hide with X, press Ctrl+Alt+C; the panel opens with the shared text and success feedback. The initial report of failure was an unclear test expectation, not a reproduced dispatch failure. This confirms Share only. Direct Copy followed by paste, empty Copy preservation, occupied bindings, and recording active bindings without transfers remain unconfirmed in this combined milestone.

## Update Triggers

- New actions, notification behavior, portal support, or registration/rollback changes.
- Native shortcut acceptance or a discovered focus/dispatch failure.

## Related Docs

- [Desktop prototype](desktop-prototype.md)
- [Shared configuration](shared-configuration.md)
- [Testing](testing.md)
- [Implementation plan](implementation-plan.md)
