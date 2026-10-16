# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

## [0.4.0] - Unreleased

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
- Version 0.4.0 is the prepared rework candidate; it has not been published and has no publication date yet.
- Sharing and copying remain explicit; background history updates do not overwrite the local clipboard or upload its contents.
- Existing V2 clients and the legacy agent remain available. Installing the rework does not automatically replace a running server, change SSH key restrictions, or take over startup registration.
- Windows x64 has isolated native and installed-update evidence. New macOS/Linux desktop packages and the exact production server installation remain separate release gates; see [release readiness](docs/release-readiness.md).

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
