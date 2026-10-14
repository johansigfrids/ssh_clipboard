# Desktop Temporary History

## Purpose

Document the panel's explicit shelf endpoint and history controls, with legacy behavior preserved. This is source implementation and automated/browser evidence; it does not install the new server or complete native SSH/clipboard acceptance.

## Key Files

- `src/configuration.rs`: per-profile endpoint and validated limits.
- `src/shelf.rs`, `src/session_protocol.rs`: bounded content labels on item metadata.
- `src/client/session.rs`, `src/client/session_transport.rs`: selected-item validation and SSH operations.
- `desktop/src-tauri/src/history.rs`: epoch/selection state and private frontend projection.
- `desktop/src-tauri/src/panel.rs`: commands, profile isolation, explicit clipboard orchestration.
- `desktop/src-tauri/src/clipboard.rs`: owner-thread capture/copy with endpoint budgets.
- `desktop/src/App.svelte`, `desktop/src/api.ts`, `desktop/src/style.css`: controls, drafts, confirmation, layout.

## Endpoint Selection and Compatibility

Connection settings choose Single shared item (legacy server) or Temporary history (new server). This is a per-profile draft, applied through the existing revision/atomic-save/shortcut transaction. Changing it does not probe, install, rewrite SSH keys, or touch a clipboard. Existing files without an endpoint read as legacy without writes. New profiles start as legacy. A shelf mode stores its own encoded/decode limits (64/256 MiB defaults, each at most 256 MiB), independently of retained V2 frame/recovery settings. Shelf operation timeout must be at most 300 seconds; its framing is strict.

The shared schema adds `endpoint: {protocol: "legacy"}` or `endpoint: {protocol: "shelf", max_item_bytes, max_decoded_bytes}`. Unknown modes/fields and invalid limits are preserved/rejected. In this dependency graph, a tagged unit variant skipped extra fields despite the enum's strict annotation; legacy mode uses an empty struct variant, with a regression proving extra fields cannot disappear on a later save. Schema 1 files containing newer endpoint fields are rejected as ambiguous. Older strict experimental readers reject this extended file instead of overwriting it.

The shelf mode invokes the separately selected `session-proxy`; legacy mode keeps `proxy`, V2 wire fixtures, and its single item. Server cutover and restricted-key changes require separate authorization. Switching profiles, endpoints, host/key/port, or advanced transport policy clears both preview and history. Local preference reload never captures or shares content.

## Selection, Copy, and Mutations

Refresh lists metadata and loads the selected item, retaining its ID only within the same epoch and current snapshot. If it is absent, select the newest remaining item. A restart replaces the epoch and invalidates the old selection. Rows show bounded labels, source, size, receipt time, and expiry; source is unknown for legacy-origin items. Selection fetches/previews by ID and brings the preview into view. Neither operation writes the OS clipboard.

Panel Copy fetches the selected ID again, guarded by the loaded epoch, validates the exact text/PNG and descriptor, then writes through the persistent clipboard owner. Expired/deleted/wrong/oversized items leave the local clipboard unchanged. Legacy panel Copy retains the displayed-snapshot contract. Optional background direct Copy deliberately clears the retained selection before listing so it always copies the newest item. Share captures once under the local endpoint budget and creates one upload operation; no automatic capture/replay is added.

Delete selected item and Clear history use an inline confirmation, not an immediate destructive click. The confirmation captures its epoch/item ID; any settings/history reload cancels it. Backend commands also check the settings revision and loaded epoch before sending a guarded mutation. Confirmed mutations update the local view without turning a later refresh failure into an apparent mutation failure. Server deletion cannot revoke copies already held elsewhere.

Transfers retain the existing immediate operation lock, so profile changes/direct actions cannot queue a different capture behind a transfer. Shelf actions now reuse an [owned persistent connection](persistent-sessions.md), with epoch checks and profile invalidation. The CLI retains bounded one-invocation sessions. Subscriptions, automatic reconnect/reconciliation, and Cancel remain subsequent work. History expiry appears on explicit refresh/action; this slice does not claim live subscriptions.

## Labels, Privacy, and Image Budgets

The shelf derives labels during validated upload preparation: first nonempty trimmed text line, control characters removed, at most 60 Unicode characters plus an ellipsis; empty text uses a fixed label. PNG labels use validated dimensions. Labels stay at most 256 UTF-8 bytes. No AI naming, external title lookup, payload trimming, conversion, or image resizing occurs. A label is content-derived and may itself contain sensitive text: do not log item metadata. Health/legacy PeekMeta remain free of labels and payloads.

Hiding previews removes text labels, text bodies, and thumbnails from the frontend projection, including cached previews; rows use generic Text/Image labels. The backend can retain the actual payload for an explicitly requested copy. Thumbnails are generated once for a fetched shelf preview and cached, rather than decoding a large image on every panel snapshot. A preview initially fetched while hidden may need Refresh after previews are enabled.

Capture/copy now carry the endpoint's decoded-image limit through the owner thread. PNG capture writes through an encoded-size-bounded writer, so it cannot grow an arbitrarily large PNG before checking the item limit. Image reading installs limits before decoder construction and checks native output as well as RGBA dimensions; a 16-bit PNG cannot bypass a smaller budget. Encoded bytes, dimensions, color/alpha, and native clipboard ownership remain intact.

## Evidence and Remaining Acceptance

Automated checks cover strict endpoint loading, no-write defaults, invalid/future fields, local labels/exact bytes, native 16-bit decode bounds, cached preview privacy, stable selection/epoch replacement, endpoint isolation, frontend ID/revision selection without transfers, draft-only endpoint edits, and confirmation/cancellation. Existing rendered/owner/shortcut/configuration tests continue to run. Linux process tests exercise both protocol modes and the item-aware CLI independently of the desktop.

A synthetic browser harness (ignored under `desktop/artifacts/history-preview`) exercises 20 rows and long labels without SSH, OS clipboard, or user settings. At 440x670 and 340x440, document width stays within the viewport, the content area scrolls, and the footer stays at the viewport bottom. Advanced shelf settings also fit at the minimum size. Selection moves to the preview; feedback appears before history so a long list does not hide transfer outcomes. This is browser layout evidence, not a native WebView/SSH/paste test.

The combined daily-use native pass still needs a deliberately configured new server/session key: select older text/image, copy and paste elsewhere, delete/expire it before copy and verify the local clipboard survives, restart/refresh and confirm epoch change, test direct Copy uses newest rather than an old selection, and verify hidden previews. It joins the already queued advanced-settings/diagnostics/direct-shortcut checks; do not request a fresh manual pass for every small commit. No installed server, startup registration, or release was changed here.

## Prevention Guidance

- A selected row is an item UUID plus server epoch, never an index or an implicit latest value.
- Check availability before writing the clipboard; a displayed cached preview does not prove the item still exists.
- Freeze confirmation context and cancel it on reload. Do not silently retarget an old confirmation to a restarted server.
- Direct Copy and panel Copy have different explicit targets; refreshing metadata must not make direct Copy retain an older selection.
- Keep content labels out of logs and hidden-preview IPC, even though they travel as metadata.
- Keep feedback near transfer controls and constrain scrolling to the content region at the native minimum size.

## Update Triggers

- Persistent sessions, live expiry/subscriptions, cancellation/recovery, or new representations.
- Endpoint/schema/preview policy, confirmation/copy semantics, or native acceptance results.

## Related Docs

- [Shelf client](shelf-client.md)
- [Shelf core](shelf-core.md)
- [Session protocol](session-protocol.md)
- [Shared configuration](shared-configuration.md)
- [Desktop prototype](desktop-prototype.md)
- [Implementation plan](implementation-plan.md)
