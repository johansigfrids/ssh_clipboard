# File Transfers

## Purpose

Describe explicit file/folder sharing through Explorer, Finder and the headless CLI in 0.6.0. Signed releases are being prepared; published 0.5.0 packages use session version 1 and do not support file bundles.

## Key Files

- `src/files.rs`, `src/files/filesystem.rs`: bundle codec, bounded capture, safe names and extraction.
- `desktop/src-tauri/src/clipboard.rs`, `file_cache.rs`: native file lists and retained local copies.
- `src/cli/shelf.rs`: explicit file transfer commands.

## Explorer and Finder

1. Copy one or more files or folders in Explorer/Finder.
2. Choose **Share current clipboard** or use the configured Share shortcut.
3. On another computer using the same server account, select the item and choose **Copy to this computer**. The direct Copy shortcut receives the newest item.
4. Paste normally into Explorer/Finder. The file manager handles destination conflicts.

The app transfers contents, not source-machine paths. It publishes new copy references on the receiving clipboard; it never propagates cut/move state or deletes originals. Copying an image file in a file manager transfers the file; copying pixels inside an image editor retains the existing image workflow. Unsupported or unreadable selected entries fail the entire Share; it never substitutes a filename's text representation.

Selection and preview fetch the bounded payload for an inert preview. They do not extract files or change the clipboard. Copy completes validation and extraction before publishing native references and verifies that every root was published. Failures before publication leave the clipboard untouched. A native API failure can happen after it clears or partially changes the clipboard, so potentially referenced files are retained.

## Supported Contents and Limits

Regular bytes, hidden files and empty folders are included. Overlapping selected roots are deduplicated. Traversal rejects symlinks, junctions, other reparse points and special files, including linked ancestors. Names must be UTF-8. Detectable size/mtime changes during capture refuse the selection; capture is not an atomic filesystem snapshot. Finish saving files before sharing them.

Permissions, executable bits, timestamps, resource forks, alternate streams and extended attributes are not transferred. There is no conversion, compression or archive extraction. One selection is one history item: 64 MiB by default, up to 256 MiB including its manifest, narrowed by client/server negotiation. Bounds also include 4,096 entries, 64 levels and a 1 MiB manifest. PNG decoded-pixel limits do not apply to files.

The server keeps bundles in bounded memory using ordinary history expiry, eviction, upload reservations and retry guarantees. There is no server disk storage or larger streaming mode. Existing operation deadlines still apply.

## Filenames and Local Copies

Extraction uses portable names on all platforms: forbidden/control characters become `_`, trailing dots/spaces are removed, reserved or empty names are prefixed with `_`, long names are shortened to 180 UTF-8 bytes while retaining a bounded extension, and collisions receive ` (2)`, ` (3)`, etc. Case collisions are treated conservatively; actual filesystem collisions also receive suffixes. Entry order determines suffix assignment. Original-to-received mappings appear after Copy and in CLI JSON. Trees that still exceed filesystem path limits fail rather than losing entries.

Desktop copies live beside the active settings file in `<settings-filename>.received-files`, with private UUID-scoped directories. They survive panel close, profile switches, server deletion/expiry and Quit. Unpinned copies expire after 24 hours and are removed at startup or before the next file Copy. Successful clipboard replacement starts a fresh 24-hour grace period for previously pinned copies.

The most recently published selection stays pinned until this app successfully replaces the clipboard. Uncertain native publications also stay pinned. The app does not poll the clipboard; a pin can outlive actual clipboard use. Admission is bounded to 1 GiB of file bytes and 128 completed selections, with bounded metadata/directory overhead in addition. Full caches refuse new copies rather than deleting protected/unexpired data. Incomplete private staging is removed after cancellation or at startup following a crash.

Local copies are ordinary files, without the server's memory-only privacy property. Hidden previews conceal names and rename mappings from frontend IPC. Absolute source paths and staging paths are not history metadata, previews or notification content. Cleanup only removes recognized entries inside the owned cache; unrecognized directories are preserved. Current-account compromise is outside the threat model.

## CLI

```text
ssh_clipboard shelf share-files report.pdf photos another-folder
ssh_clipboard shelf list
ssh_clipboard shelf fetch-files <item-uuid> --output-dir received
```

The output directory must not exist. The CLI validates/stages the entire selection beside it, then publishes atomically without replacement. Failures never publish a partial destination. Success prints JSON with final absolute `paths` and relative `renames`. Optional `--epoch` binds fetch to a listed epoch. Direct sharing uses the existing `--device-id` / `--device-name` rules.

Existing `share --file` and `--format png` keep their meanings. `fetch --stdout` / `--output` export exact bundle bytes for a file item; use `fetch-files` to extract it. CLI outputs are permanent exports outside the desktop cache.

## Compatibility and Acceptance

Current source negotiates session version 2 inside the unchanged SCBS bootstrap. Upgrade both clients and server; version 1 peers are rejected. Settings schema 3, device identities and managed-server schema 2 remain unchanged. Source implementation does not imply deployment or publication.

Automated tests cover codec, filesystem, shelf/session, CLI process, cache and fake native-adapter contracts. Native acceptance requires Windows-to-Windows, Windows-to-Mac, Mac-to-Windows and Mac-to-Mac Explorer/Finder paste, hash/tree comparison, shortcuts, collisions/renames, and repeated paste after hide/Quit/reopen. These checks are separate from rendered UI tests; see [testing](testing.md).

## Prevention Guidance

Read native file lists before text/images; distinguish absence from read failure. Freeze bytes before upload and never reopen sources during retry. Native APIs can omit unconvertible paths, so preflight and read back every published root. The arboard Windows file-list setter retains other formats: explicitly clear them first so stale cut/move intent cannot survive. Probe native file-format presence because the dependency can report malformed data as unavailable. Close staging handles before directory publication on Windows, and use a no-replace native rename: `std::fs::rename` can replace an empty directory there. Preserve durable pins across native failures/crashes.

## Update Triggers

- Bundle format, limits, metadata, native publication, filename mapping or cache retention changes.

## Related Docs

- [Protocol](session-protocol.md), [shelf core](shelf-core.md), [history](desktop-history.md), [recovery](share-recovery.md), [CLI](cli.md), [compatibility](compatibility-and-cutover.md).
