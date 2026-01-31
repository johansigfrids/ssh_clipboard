# Autostart (Start at Login)

## Purpose
Document how Phase 4 “start at login” works and what `autostart refresh` does.

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
Autostart is implemented via `auto-launch` and points to the current `ssh_clipboard` executable path with the `agent` subcommand.

## Handling moved/updated binaries
If you move or replace the `ssh_clipboard` binary after enabling autostart, the stored autostart entry might point to the old path.

Recommended workflow:
- Run `ssh_clipboard autostart refresh` after moving/updating the binary.
- The agent also attempts a best-effort refresh at startup if `autostart_enabled` is set in config.

If the current executable path cannot be resolved to a usable absolute path, autostart may fail; disable and re-enable autostart after fixing the installation location.

## Platform notes
Exact mechanisms differ per OS:
- Windows: typically a per-user startup entry.
- macOS: typically a per-user Launch Agent.

## Update Triggers
- Changes to autostart strategy or arguments (`agent` flags).
- Changes to how config stores `autostart_enabled`.

## Related Docs
- `docs/agent.md`
- `docs/cli.md`
- `docs/security.md`

