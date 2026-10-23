# File Transfer Source Acceptance - 2026-10-23

## Purpose

Record bounded verification of the uncommitted file-transfer implementation, including native adapters and isolated Windows/Mac desktop app checks. Release and installer acceptance are outside this record. Installed applications and the managed server were not replaced.

## Key Files

- `src/files.rs`, `src/files/filesystem.rs`: bundle and filesystem behavior.
- `desktop/src-tauri/src/clipboard.rs`: production native adapter and opt-in fixture probe.
- `desktop/src-tauri/src/panel/ipc_contract_tests.rs`: platform-neutral preview fixture.

## Platform Checks

Windows and Linux/WSL core tests, desktop Rust tests, frontend/tooling tests, Svelte/Vite build, Rust formatting, Clippy and documentation checks passed during implementation. The explicit live SSH fixture remains ignored without its disposable endpoint.

After owner approval, the source and built frontend were copied to a disposable Apple Silicon Mac directory. Core tests/Clippy and desktop all-feature tests/Clippy passed natively with the configured deployment target. The Mac run exposed a test-only normalization omission: the new hidden-file preview fixture retained `platform=macos`, while the shared fixture intentionally uses Windows. Normalizing this field and shortcut availability fixed the cross-platform fixture; application behavior was unchanged.

The native clipboard probe is ignored by default and requires an explicit owned fixture marker and mode. Capture refuses unrelated clipboard files. It calls the actual clipboard worker, bundle capture, cache preparation and native publication code, without opening production settings or connecting to a server. Its published cache is intentionally retained after process exit so file-manager paste can be tested.

## Native Results

| Check | Result |
| --- | --- |
| Explorer Copy of the synthetic source folder, then native Windows capture | Passed: three files and two folders, including hidden data, a Unicode filename, arbitrary binary bytes and an empty folder. |
| Native Windows publication, process exit, then Explorer Paste | Passed: all file hashes and the empty folder match; source files remain. |
| Fresh-process Windows cache reopen and reference validation after paste | Passed. |
| Transfer captured bundle to Mac, extract and publish native file URLs | Passed; complete native file list verified. |
| Fresh-process Mac cache reopen and reference validation | Passed. |
| Owner-performed Finder Paste of the Windows-origin selection | Passed: owner confirmed the paste; independent checks verified all three file hashes, the Unicode filename, hidden file and empty folder. |
| Owner-performed Finder Copy of the pasted source folder, then native Mac capture | Passed: all 377 encoded bytes match the original Windows bundle, including file contents, names and tree structure. |
| Native Mac publication of that Finder-copied bundle, process exit/cache reopen, then owner-performed Finder Paste | Passed: all three file hashes and the empty folder match; the copied source remains. |
| Transfer the Finder-copied bundle to Windows, publish native references, then Explorer Paste | Passed: all three file hashes, the Unicode filename, hidden file and empty folder match. |
| Capture the owned published Mac references and return the bundle to Windows | Passed: identical encoded bundle bytes. |
| Native Windows publication of the returned bundle, then a second Explorer Paste | Passed: all file hashes and the empty folder match again. |
| Full desktop Share/Copy/global shortcuts | Passed on Windows and Mac in the isolated full-app checks below. |

The 377-byte original, Mac-recaptured and Finder-copied bundles all have SHA-256 `4618f64451e2e0c8e9e7719e4124ea3cbd6e9b62da1b40d16feaae15a5f27e4a`. SCP transported the synthetic bundle between computers; the separate CLI/service process tests establish shelf transport behavior. These native results must not be described as a full GUI/SSH acceptance campaign.

This establishes file-manager input/output through the production native adapter and bundle/cache paths for Windows-to-Windows, Windows-to-Mac, Mac-to-Windows and Mac-to-Mac. Windows UI automation used a separate Explorer tab and restored the original tabs. Republish the fixture after automated address entry: text-entry automation can replace the clipboard and otherwise invalidate the file-paste setup.

Finder automation did not complete and was stopped. A separate AppleScript alias clipboard experiment exposed only the legacy `alias` format, rather than file URLs; the guarded capture correctly refused it. This does not demonstrate a Finder-copy defect. The Mac test destination was subsequently opened normally, with the validated synthetic file URLs still on its clipboard. The owner confirmed that Finder Paste worked, and independent filesystem checks verified the resulting hashes and empty folder.

## Source Identity and Retained Evidence

The following critical source hashes matched the local workspace and the tested Mac snapshot after the fixture changes:

| Source | SHA-256 |
| --- | --- |
| `src/files.rs` | `04b917bf28521c6ac55f9a81f7199c6b14c2ea9061542e135c6daf431c5995ec` |
| `src/files/filesystem.rs` | `9483b73af53e9c484505074c2a103e8c7a1a1fccd949a3beda3cc8782e18ce83` |
| `desktop/src-tauri/src/clipboard.rs` | `bdffeabd5d5ed41579e6df925b81e2276daa7cc22f45d48f4267396b9d869edf` |
| `desktop/src-tauri/src/file_cache.rs` | `de39c7d7929cf1698b73b5a4fb8c5e308b5673f0d89d0e94f57530487d8a9e95` |
| `desktop/src-tauri/src/panel/ipc_contract_tests.rs` | `3359601ab537281c8fcc5d47fcd98698ab6754fe59ed575ba974eae5365e136b` |

Ignored local evidence is consolidated in `target/file-transfer-acceptance/`, including the native summary, core/desktop logs, per-step Mac results and cleanup manifests. Tiny native-reference caches remain at their original fixture paths because references survive the publishing process. Disposable source copies, binaries, build caches and pasted fixtures were subsequently removed as recorded below.

## Isolated Full-App Preparation

At the owner's request, a disposable Windows source snapshot was built for the remaining full-app checks. Only its settings-store function and Tauri product/identifier/window title differ from the working source. The settings-store function points exclusively to the owned fixture; startup and update features are disabled. Its transport setting selects a small local wrapper connecting the real session client to a disposable WSL daemon/proxy, with no SSH trust changes or installed server replacement. This is a transport substitution, not evidence of live SSH routing.

The Windows standalone executable has SHA-256 `8ce75ff862434b99cbf35063c529dad3bcd71189bc5784a24ed1c9a019235879`. Its embedded-frontend integration check passed. Computer Use initially timed out waiting for app-control approval while the owner was away. After the owner returned and approved access, the Windows full-app checks below passed. The initial disposable daemon expired during setup and was restarted before successful transfers; verify fixture health and allow enough lifetime for supervised checks. The production settings hash remained unchanged.

An independent real-filesystem check captured synthetic Linux names into the disposable shelf and fetched them with the Windows CLI. Reserved `CON.txt` became `_CON.txt`, case-colliding `README`/`Readme` gained a suffix, forbidden-character collisions received deterministic suffixes, and a trailing dot was removed. Two completed exports had identical rename mappings and file bytes, retained the hidden file and empty folder, and a fetch into an existing destination was refused. One additional export failed at the final Windows directory move with `Access denied`; retry into that still-new destination succeeded. This bounded result does not establish the cause of that transient failure. The initial verification script also required case-sensitive comparisons for the intentional case collision; that was a harness correction.

### Windows Full-App Results

| Check | Result |
| --- | --- |
| Explorer Copy, then panel Share current clipboard | Passed: shelf fetch returned the exact original 377-byte bundle. |
| File preview before Copy | Passed: file/folder counts, size and source name displayed; zero staging copies existed. |
| Panel Copy to this computer, then Explorer Paste | Passed: all three hashes, hidden/Unicode names and empty folder matched; one completed staging copy appeared. |
| Repeated paste after window Close/hide | Passed with matching file hashes and empty folder. |
| Open-panel global shortcut | Passed: Ctrl+Alt+Shift+F8 reopened the hidden panel. |
| Quit through the app, then paste with the process absent | Passed with matching file hashes and empty folder. |
| Relaunch the app, then paste without another Copy | Passed with matching file hashes and empty folder. |
| Background Share global shortcut | Passed: Ctrl+Alt+Shift+F9 uploaded a distinct four-file folder, and the panel opened with success feedback. |
| Background Copy global shortcut, then Explorer Paste | Passed: Ctrl+Alt+Shift+F10 received the newest four-file selection; all four hashes and the empty folder matched. |
| Disable previews in Settings > Behavior | Passed: shared-item/history names and rename mappings were concealed. Changing preview settings created no staging copies. |
| Hide panel after copying | Passed: successful Copy hid the panel and retained usable file references; subsequent paste matched all four hashes and the empty folder. |
| Copy incompatible names with hidden previews | Passed: no filenames or rename mappings appeared in panel feedback. |
| Copy incompatible names with visible previews | Passed: all five original-to-received mappings appeared in the bounded preview, without staging paths. |
| Paste the renamed selection | Passed: seven file hashes and empty folder matched the independent CLI export. |
| Paste again into the same destination | Passed: Explorer reported seven conflicts; Skip preserved every destination hash. |

The test app was quit through its own control, the owned WSL daemon was stopped, and the user's original Explorer tabs were restored. Completed staging directories were retained, including the final published selection's durable pin. These results establish the Windows source app with substituted local transport, not installed startup/updater behavior or live SSH transport. Explorer input must focus its file-list pane before pasting; window activation alone can retain address-bar focus.

### Mac Full-App Results

A separate Mac source app was built with the same settings-store/product isolation and passed the embedded-frontend integration check. Its standalone executable has SHA-256 `b6fe835cf72c99e6d21bf56a51eb969dab7abcd001c922cb7d882c65572c4da6`. A fixture-only helper binary runs the production `ShelfService` on a private Unix socket, bounded to one hour; a configured local `nc` wrapper connects the unchanged client session path. The private socket directory is mode 0700 and socket mode 0600. This bypasses SSH only for the local app check and does not claim Linux listener authentication or installed server behavior on Mac. Production app/settings are not replaced.

The Mac fixture connection passed CLI Health. The owner exercised Finder and the test app one step at a time; independent filesystem and shelf checks verified each resulting payload/tree.

| Check | Result |
| --- | --- |
| Finder Copy, then panel Share current clipboard | Passed: owner saw three files/two folders and success; shelf fetch matched the original 377-byte bundle exactly. |
| Preview before Copy | Passed: no received staging directories existed; only cache owner/lock files were present. |
| Panel Copy to this computer, then Finder Paste | Passed: owner saw success and pasted the source folder; all three hashes, hidden/Unicode names and empty folder matched. |
| Window Close/hide, then paste again | Passed: process remained running and all three hashes/empty folder matched. |
| Open-panel global shortcut | Passed: Cmd+Option+Shift+F8 reopened the hidden test panel. |
| Quit through the app, then paste with the process absent | Passed: process exit and all three pasted hashes/empty folder were verified. |
| Relaunch the app, then paste without another Copy | Passed with matching hashes and empty folder. |
| Background Share global shortcut | Passed: Cmd+Option+Shift+F9 from Finder uploaded the distinct four-file selection and opened the panel with success; all four exported hashes matched. |
| Background Copy global shortcut, then Finder Paste | Passed: Cmd+Option+Shift+F10 opened the panel, produced a new pinned completed copy, and pasted all four files/empty folder with matching hashes. |
| Paste again into the same destination | Passed: Finder offered Stop/Replace; owner chose Stop and every destination hash remained unchanged. |
| Copy seven separate files and an empty folder as eight roots | Passed: all roots were staged/published; the previous copy was unpinned and only the new completed copy remained pinned. |
| Reserved, forbidden, trailing-dot and case-colliding names | Passed: owner saw Received names; actual names mapped to `_CON.txt`, `alpha_.txt`, `alpha_ (2).txt`, `Readme (2)` and `trailing` as expected. |
| Finder Paste of that renamed multi-root selection | Passed: seven file hashes, including hidden/binary bytes, and the empty folder matched. |

The four-file background-Share bundle has SHA-256 `b1b21de359bb87172c9b37b2e1e906b0b3911ada5e8c9459009f4790506091db`. After the final owner-performed Quit, the test process was absent, the owned private service was stopped, and only its validated private socket/directory were removed. Production settings hashes remained unchanged on both computers. The Mac retains three completed staging copies with one durable pin; Windows retains five with one pin. Published copies and earlier native-probe caches were preserved.

The retained acceptance archive includes Windows build logs, JSON mappings, no-overwrite error evidence and per-step Mac results. Source snapshots and executable test fixtures were removed after acceptance at the owner's request.

## Post-Acceptance File Cleanup

The owner explicitly requested removal of disposable test files on Windows, Mac and WSL while preserving acceptance records and clipboard-backed files.

- Windows: removed the two fixture directories' disposable source trees, executables, helpers, synthetic sources/paste destinations and copied source archive. Consolidated records into `target/file-transfer-acceptance/`. Normal repository Cargo caches were preserved.
- Mac: removed disposable source, binaries, Cargo build caches and synthetic sources/paste destinations from the owned temporary workspace. Kept its `acceptance-records/` directory and three native-reference caches at their original paths. Allocated workspace size fell from about 6.1 GiB to 308 KiB before the final record refresh.
- WSL: removed the exact owned `/tmp/ssh-clipboard-fileapp-20261023` fixture after confirming its daemon was stopped. Normal toolchain/build output was preserved.

All five clipboard-cache trees were preserved conservatively, including small unpinned records. Before/after SHA-256 manifests verified that all 54 Windows cache files (1,284 bytes including metadata) and 31 Mac cache files (706 bytes including metadata) remained unchanged. Empty folders and native paths were retained. These fixture caches have no running cleanup process; they can be explicitly removed after their file clipboards have been replaced. The cleanup manifests are retained with the acceptance evidence. No production settings, installed application, managed server or working source was removed.

## Remaining Boundaries

The bounded native acceptance described above is complete. Full-app runs substituted local transport, while cross-computer adapter bundles used SCP; this does not establish one live SSH-connected GUI session between the computers. Windows additionally exercised privacy settings and automatic hide-after-copy; those preference controls were not manually repeated on Mac. Automated cancellation, quota, native-failure and recovery checks remain distinct from these native passes. No installed startup/updater behavior, release publication or production server migration was tested.

## Update Triggers

- Record subsequent owner-performed Finder or desktop UI checks with their exact source boundary.

## Related Docs

- [File transfers](../file-transfers.md), [testing](../testing.md), [desktop development](../desktop-development.md).
