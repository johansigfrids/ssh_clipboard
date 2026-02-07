# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]
### Added

### Changed

### Fixed

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
