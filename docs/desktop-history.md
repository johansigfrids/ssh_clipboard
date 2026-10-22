# Desktop Temporary History

## Purpose

Document the panel's explicit shelf endpoint and history controls, for the session-only desktop. Automated/browser checks and a bounded Windows native pass provide separate evidence; neither installs the new server or completes production cutover.

## Key Files

- `src/configuration.rs`: per-profile endpoint and validated limits.
- `src/shelf.rs`, `src/session_protocol.rs`: bounded content labels on item metadata.
- `src/client/session.rs`, `src/client/session_transport.rs`: selected-item validation and SSH operations.
- `desktop/src-tauri/src/history.rs`: epoch/selection state and private frontend projection.
- `desktop/src-tauri/src/panel.rs`: commands, profile isolation, explicit clipboard orchestration.
- `desktop/src-tauri/src/clipboard.rs`: owner-thread capture/copy with endpoint budgets.
- `desktop/src/App.svelte`, `desktop/src/api.ts`, `desktop/src/style.css`: controls, drafts, confirmation, layout.
- `desktop/src/history-format.ts`: precision-checked local time and byte-size presentation.

## Endpoint Selection and Compatibility

All profiles use `session-proxy`. Saved plain session limits default to 64 MiB encoded and 256 MiB decoded; changing a profile or connection policy clears history, previews and session ownership through the guarded Apply transaction. There is no mode selector, protocol fallback or separate snapshot cache. Server replacement and restricted-key routing remain explicit rollout actions.

## Selection, Copy, and Mutations

When a new item arrives, metadata updates select the newest item so Panel Copy is ready to copy it. Ordinary refreshes and expiry/deletion updates retain a manually selected ID within the same epoch and current snapshot. If it is absent, select the newest remaining item. A restart replaces the epoch and invalidates the old selection. Refresh lists metadata and loads the selected item; passive updates clear an old preview when selection changes and do not fetch the new payload. Rows show bounded labels, source, size, receipt time, and expiry; source is unknown when wire metadata omits an origin. Explicit selection fetches/previews by ID and brings the preview into view. These operations do not write the OS clipboard.

Panel Copy fetches the selected ID again, guarded by the loaded epoch, validates the exact text/PNG and descriptor, then writes through the persistent clipboard owner. Expired/deleted/wrong/oversized items leave the local clipboard unchanged. Optional background direct Copy deliberately clears the retained selection before listing so it always copies the newest item. Share captures once under the local endpoint budget and creates one upload operation; no automatic capture/replay is added.

Delete selected item and Clear history use an inline confirmation, not an immediate destructive click. The confirmation captures its epoch/item ID; any settings/history reload cancels it. Backend commands also check the settings revision and loaded epoch before sending a guarded mutation. Confirmed mutations update the local view without turning a later refresh failure into an apparent mutation failure. Server deletion cannot revoke copies already held elsewhere.

Transfers retain the existing immediate operation lock, so profile changes/direct actions cannot queue a different capture behind a transfer. Shelf actions now reuse an [owned persistent connection](persistent-sessions.md), with epoch checks and profile invalidation. The CLI retains bounded one-invocation sessions. A separate bounded metadata owner now snapshots/watches history and recovers transient failures automatically; trust/authentication/protocol errors pause for explicit action. History/expiry updates never fetch payloads or overwrite the local clipboard. Background replies are guarded by profile, revision, monitor generation, sequence, and the latest user-operation barrier. Cancel is available for active user operations; uncertain uploads now have [read-only acknowledgment checks and explicit captured retry/discard](share-recovery.md), with original epoch and server-deadline guards.

## Labels, Privacy, and Image Budgets

History rows show relative receipt/expiry labels and binary sizes (B, KiB, MiB, and larger units). Each row has a separate keyboard-accessible Exact times disclosure with localized Received/Expires values and machine-readable timestamps. Selection remains a button with pressed state; the disclosure is not nested inside it. Selecting a row focuses the preview heading without copying anything.

Relative labels update every 30 seconds only while the panel/document is visible and history exists. Settings navigation, hiding and unmount stop the timer. Ticks never fetch, reorder, select, remove, or recreate rows keyed by epoch plus item ID; open disclosures and focus survive. Unknown, unsafe-integer or invalid dates remain unknown. A future receipt reports clock disagreement, and elapsed expiry says Past expiry (local clock): only server metadata establishes membership.

Passive removal recovers focus to the history heading (or main content if history disappears) only when the previously focused row/control was removed and focus has fallen to the document body. It never moves an existing user focus. A separate polite status announces that loss; relative-time ticks are not live announcements. Confirmations focus their action; Escape/Cancel restores the invoker, and successful deletion focuses the history heading. See [keyboard evidence](history/desktop-ux-plan.md#pr-6-history-scanning-and-keyboard-usability).

The shelf derives labels during validated upload preparation: first nonempty trimmed text line, control characters removed, at most 60 Unicode characters plus an ellipsis; empty text uses a fixed label. PNG labels use validated dimensions. Labels stay at most 256 UTF-8 bytes. No AI naming, external title lookup, payload trimming, conversion, or image resizing occurs. A label is content-derived and may itself contain sensitive text: do not log item metadata. Health remain free of labels and payloads.

Hiding previews removes text labels, text bodies, and thumbnails from the frontend projection, including cached previews; rows use generic Text/Image labels. The backend can retain the actual payload for an explicitly requested copy. Thumbnails are generated once for a fetched shelf preview and cached, rather than decoding a large image on every panel snapshot. A preview initially fetched while hidden may need Refresh after previews are enabled.

Capture/copy now carry the endpoint's decoded-image limit through the owner thread. PNG capture writes through an encoded-size-bounded writer, so it cannot grow an arbitrarily large PNG before checking the item limit. Image reading installs limits before decoder construction and checks native output as well as RGBA dimensions; a 16-bit PNG cannot bypass a smaller budget. Encoded bytes, dimensions, color/alpha, and native clipboard ownership remain intact.

## Evidence and Remaining Acceptance

PR 4 compacts the panel while preserving Copy above Share. Copy is primary when a snapshot or selected history item is available; otherwise Share is primary and disabled Copy is neutral. Refresh history sits beside the history heading, remains available for an empty history, and retains the same revision-guarded `refresh_item` command. Nothing about moving these controls changes selection, metadata watching, or clipboard access.

Preview labels are visually bounded to two lines, text previews to 112 CSS pixels, and image previews to 128 CSS pixels. These display limits do not modify stored/copied content. Main text sizing is unchanged. Ordinary empty and short-text states show transfer actions and the history heading/first row at the default 440 x 670 viewport. Recovery/errors may take priority and remain fully available through the main scroll area. At minimum size, actions remain reachable by scrolling/focus.

The 2026-10-18 synthetic browser pass covers empty, short-text, hidden-preview, image, long-label, unchecked, legacy, unconfigured, busy/Cancel, and uncertain-share states in both themes at 440 x 670 and 340 x 440. All forty layouts retain bounded shell/header/content width; long labels cannot enlarge the grid column. In the short-text fixture, the first row's bottom moves from y=673 (past the old content bottom y=632) to y=547 (inside the new bottom y=629). See [screenshots and delivery evidence](history/desktop-ux-plan.md#pr-4-compact-layout-and-consistent-buttons). This uses mock IPC and does not add native clipboard/SSH evidence.

Automated checks cover strict endpoint loading, no-write defaults, invalid/future fields, local labels/exact bytes, native 16-bit decode bounds, cached preview privacy, stable selection/epoch replacement, endpoint isolation, frontend ID/revision selection without transfers, draft-only endpoint edits, and confirmation/cancellation. Existing rendered/owner/shortcut/configuration tests continue to run. Linux process tests exercise both protocol modes and the item-aware CLI independently of the desktop.

A recorded synthetic browser harness used the ignored `desktop/artifacts/history-preview` directory to exercise 20 rows and long labels without SSH, OS clipboard or user settings. That generated fixture directory need not exist in a clean checkout. At 440x670 and 340x440, document width stays within the viewport, the content area scrolls, and the footer stays at the viewport bottom. Advanced shelf settings also fit at the minimum size. Selection moves to the preview; feedback appears before history so a long list does not hide transfer outcomes. This is browser layout evidence, not a native WebView/SSH/paste test.

The [2026-10-16 Windows native pass](history/native-history-acceptance.md) confirmed older selected text in Notepad, 480 x 240 PNG in Paint, explicit Share, passive remote changes without changing selection/clipboard, delete confirmation/cancellation, hidden-preview Copy, legacy/shelf switching and direct Copy of newest. Import review/relaunch, reconnect and cancelled-Share acknowledgment recovery also passed in the isolated portable build over real SSH. Clear confirmation, removed/expired selected-Copy refusal and server-restart epoch UI were not manually exercised. The session is closed; use automated fault evidence next and keep installed cutover/other platforms separate.

## Prevention Guidance

- A selected row is an item UUID plus server epoch, never an index or an implicit latest value. Detect arrivals by comparing the new head with the previous item IDs, rather than treating every sequence change as an arrival: expiry/deletion also advances the sequence. Retain a cached preview only when both epoch and selected ID stay unchanged.
- Check availability before writing the clipboard; a displayed cached preview does not prove the item still exists.
- Freeze confirmation context and cancel it on reload. Do not silently retarget an old confirmation to a restarted server.
- Direct Copy and panel Copy have different explicit targets; refreshing metadata must not make direct Copy retain an older selection.
- Keep content labels out of logs and hidden-preview IPC, even though they travel as metadata.
- Keep feedback near transfer controls and constrain scrolling to the content region at the native minimum size.
- Check the grid's actual bounds with long unbroken connection targets. A hidden document scrollbar can mask a widened implicit column; bound both the grid column and content row.
- Keep empty-history Refresh enabled and revision-guarded. Visual priority and preview truncation must not change the item/epoch copied or the original payload.
- Validate timestamp strings before Number conversion; JavaScript rounding must not turn unknown server metadata into a credible exact time.
- Keep local-clock display independent of server expiry and DOM identity. Do not announce clock ticks or steal focus on ordinary metadata updates.
- Keep history hover on the row surface and suppress the generic button hover border/background inside it. Nested rounded button borders split the selected card visually; the Exact times disclosure remains a separate control and keyboard focus stays visible.

## Update Triggers

- Persistent sessions, live expiry/subscriptions, cancellation/recovery, or new representations.
- Endpoint/schema/preview policy, confirmation/copy semantics, or native acceptance results.

## Related Docs

- [Shelf client](shelf-client.md)
- [Shelf core](shelf-core.md)
- [Session protocol](session-protocol.md)
- [Shared configuration](shared-configuration.md)
- [Desktop development](desktop-development.md)
- [Implementation plan](history/implementation-plan.md)
