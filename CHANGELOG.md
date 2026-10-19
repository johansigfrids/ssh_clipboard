# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.4.3] - 2026-10-19

### Changed
- Keep settings Apply/Discard actions visible across all sections, retain unsaved drafts during navigation, and apply Start at login with the rest of the settings transaction.
- Compact the desktop panel, group behavior and shortcut preferences, and keep feedback relevant while preserving actionable errors and transfer results.
- Improve history timestamps, exact-time details and keyboard navigation.
- Show actionable desktop update states and version-specific release notes from reviewed source files.
- Default source builds provide the native clipboard CLI without the retired tray agent. The explicit `agent` compatibility feature, V2 routes and settings import remain available through 0.4.
- Close the delivered Windows/Apple Silicon/Linux-server implementation plan; defer Linux desktop and further rare-scenario manual verification.

### Fixed
- Suppress Windows SSH transport console windows while preserving piped input/output and child-process cleanup.

### Removed
- Retire the obsolete automatic `v*` CLI/agent publisher. Protected signed desktop/server workflows and existing published assets are unchanged.

### Release status
- Published as `desktop-v0.4.3` and repository latest after protected Windows/Apple Silicon signing, independent package admission, owner-confirmed Windows text Share/Copy and settings checks, and separate publication approval. Anonymous delivery of all twelve assets and the combined keyed feed passes. Signed source remains `79205abfa5bf5459846cbe427d0ce13abc34f166`; Linux server 0.4.0 and earlier desktop assets are unchanged. See [acceptance](docs/history/release-0.4.3-acceptance.md) for exact evidence and native limits.

## [0.4.2] - 2026-10-16

### Added
- Apple Silicon application archive/signature inspection and a release-profile package probe using disposable trust, with a documented clean migration from the legacy Mac client.
- Protected Windows/macOS desktop staging with a shared update feed that requires both platform packages before draft creation. Clean Mac migration, native text/login/shortcut acceptance and an isolated real update/restart rehearsal pass.

### Release status
- Published as `desktop-v0.4.2` and repository latest after protected Windows/Apple Silicon signing, independent package admission, Mac acceptance and separate owner approval. Anonymous delivery of all twelve assets and the combined keyed feed passes. Signed source remains `b465c4407805f4feb8d4f58cb9d28e209e26ed7e`; later acceptance/docs do not replace package bytes. Earlier Windows 0.4.1 and Linux server 0.4.0 assets are unchanged.

## [0.4.1] - 2026-10-16

### Fixed
- Quote the installed Windows startup executable independently of its arguments, including paths with spaces or Unicode. Keep startup enablement in the current user profile.

### Release status
- Windows desktop published as `desktop-v0.4.1` after independent signature verification and installed upgrade/startup acceptance. Anonymous public installer/feed verification passes. The immutable 0.4.0 desktop draft is retained as acceptance evidence and will not be overwritten or published. The independently verified server-v0.4.0 was published under its separate approval after clean live installation and public-delivery acceptance.

## [0.4.0] - 2026-10-16

### Added
- Experimental desktop quick panel with visible Share/Copy actions, text/image previews, tray access, and discoverable optional shortcuts.
- Shared saved connection profiles, settings UI, metadata-only diagnostics, and backed-up legacy settings migration with an explicit import review.
- Bounded temporary Linux clipboard history with item selection, expiry, confirmed deletion/clearing, and an explicit compatibility socket for legacy clients.
- Reused SSH sessions, live metadata updates, cancellation, and explicit recovery of uncertain shares using the original capture.
- Signed Windows desktop package/update staging and signed Linux server maintenance with verified health, retained releases, and recovery after failed activation.

### Changed
- Separate headless server/stdio builds from OS clipboard, tray, and hotkey dependencies.
- Shelf profiles negotiate encoded/decoded limits; legacy profiles retain their existing endpoint and explicit limits.
- Production desktop package evidence now binds the reviewed clean source commit, release profile, matching CLI, and exact package size/hash before draft creation.

### Fixed
- Keep visible update details in place during rechecks instead of flashing or jumping the page.
- Preserve background startup without a panel flash and retain settings across installed updates.
- Bound competing subprocess fault fixtures together so parallel test modules do not exhaust each other's timing budgets.

### Migration and release status
- Linux x86_64 server 0.4.0 is published after independent signature verification and clean live installation/SSH acceptance. Anonymous public-delivery checks pass. The superseded Windows desktop 0.4.0 remains a draft; desktop 0.4.1 was the repository's latest release at server publication.
- Sharing and copying remain explicit; background history updates do not overwrite the local clipboard or upload its contents.
- Existing V2 clients and the legacy agent remain available. Installing the rework does not automatically replace a running server, change SSH key restrictions, or take over startup registration.
- Windows x64 has native and installed-update evidence; the Linux server has exact signed persistent-installation and public-delivery evidence. New macOS/Linux desktop packages and actual server reboot/last-session logout retain their separate gaps; see [release readiness](docs/history/release-readiness.md).

## [0.3.0] - 2026-02-07
### Added
- Background agent support and related agent command surface (`agent`, setup/autostart flow improvements).
- Added `install-client` / `uninstall-client` flow and expanded `doctor` diagnostics.

### Changed
- Linux Wayland behavior now auto-disables global hotkeys with an explicit startup notice.
- macOS notification backend behavior changed to an `osascript`-based path.
- Replaced `bincode` with `wincode` for protocol payload serialization while keeping protocol version `2` wire format stable with explicit fixture tests.
- Upgraded direct dependencies to latest compatible versions (`clap`, `notify-rust`, `proptest`, `thiserror`, `time`, `winreg`).

### Fixed
- Cross-platform `clippy` and `cfg` hygiene issues.

## [0.2.0] - 2026-02-03
### Added
- Proxy auto-start support and improved daemon socket checks.
- Agent config management (`config set`) and one-command setup (`setup-agent`).
- Linux daemon setup (`install-daemon`) and uninstall helper.
- Real app/tray icon assets and Windows icon embedding.
- Separate macOS Intel and Apple Silicon release artifacts.

### Changed
- Updated default hotkeys for better OS-specific ergonomics.

## [0.1.0] - 2026-02-01
### Added
- CLI commands for push, pull, and peek.
- Linux daemon and proxy modes (local UNIX socket).
- Framed protocol with versioning and size limits.
- Clipboard support for text and PNG images.
- Background agent with tray + global hotkeys (Windows/macOS/Linux).
- SSH transport using the system `ssh` binary.
