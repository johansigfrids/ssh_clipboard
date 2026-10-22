# Build and Dependency Boundaries

## Purpose

The core has one headless build; native desktop integration belongs to the desktop.

## Key Files

- `Cargo.toml`: headless library/CLI.
- `desktop/src-tauri/Cargo.toml`: native desktop dependencies.
- `src/client/image.rs`: shared bounded PNG codec.

## Builds

`cargo build --release` builds the session CLI and Linux server commands. There are no `agent` or `system-clipboard` features or agent binary. CLI transfers use explicit file/stdin/stdout modes. The desktop owns native clipboard/tray/shortcut/notification/startup dependencies and a worker retaining clipboard ownership until Quit.

The desktop links the core with `external-resource-owner` so executable resources have one owner. This is a resource boundary, not a second protocol/build product. Root code shares PNG validation and RGBA data without OS clipboard types. PNG is the sole image representation; do not enable unrelated image codecs without a requirement.

The installed build bundles the headless CLI. CI checks the default root runtime tree for clipboard, tray, hotkey, GTK, X11/Wayland and registry integration dependencies. Desktop dependencies legitimately include those needed for native behavior. Windows/macOS/Linux core jobs use the same Cargo commands.

## Update Triggers

- Manifest features, runtime dependency ownership, sidecar builds or supported formats change.

## Related Docs

- [Testing](testing.md), [CI](ci.md), [desktop development](desktop-development.md).
