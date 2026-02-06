# Autostart (Start at Login)

## Purpose
Document how “start at login” works and what `autostart refresh` does.

## Key Files
- `src/agent/autostart.rs`
- `docs/agent.md`
- `docs/cli.md`

## Commands
```
ssh_clipboard autostart status
ssh_clipboard autostart enable
ssh_clipboard autostart disable
ssh_clipboard autostart refresh
```

## Scope
- Autostart is **per-user** (not system-wide).

## How it works
Autostart is implemented via `auto-launch` and points to the `ssh_clipboard_agent` binary (same folder as `ssh_clipboard`).
The autostart app name remains `ssh_clipboard` to avoid stale entries across upgrades.

`autostart enable/disable/refresh` also updates the agent config field `autostart_enabled` (used for tray state and best-effort refresh at agent startup).

## Handling moved/updated binaries
If you move or replace the binaries after enabling autostart, the stored autostart entry might point to the old path.

Recommended workflow:
- Run `ssh_clipboard autostart refresh` after moving/updating the binary.
- The agent also attempts a best-effort refresh at startup if `autostart_enabled` is set in config.
- `ssh_clipboard autostart status` and `ssh_clipboard autostart disable` can still be used even if the agent binary is currently missing.

If the current executable path cannot be resolved to a usable absolute path, autostart may fail; disable and re-enable autostart after fixing the installation location.

## Platform notes
Exact mechanisms differ per OS:
- Windows: typically a per-user startup entry.
- macOS: typically a per-user Launch Agent.
- Linux: XDG autostart entry under `~/.config/autostart/`.

## Update Triggers
- Changes to autostart strategy or arguments (`agent` flags).
- Changes to how config stores `autostart_enabled`.

## Related Docs
- `docs/agent.md`
- `docs/cli.md`
- `docs/security.md`
