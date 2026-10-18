# CLI Reference

## Purpose
Concise command/flag reference for the `ssh_clipboard` CLI.

## Key Files
- `src/main.rs`
- [README.md](../README.md)

## Current and Retired Commands

Default source builds include native clipboard transfers and shared `settings`. Old `agent`, `config`, `autostart`, `setup-agent`, `install-client` and `uninstall-client` commands require explicit `--features agent` or a frozen legacy archive. They are compatibility tools, not the normal desktop setup path. V2 transfers/proxy and settings migration remain supported through 0.4; see [retirement](legacy-retirement.md).

## Commands

### Linux Shelf

The cross-platform `shelf` namespace provides `list`, `health`, `share --file|--stdin`, `fetch <id> --stdout|--output`, `delete <id> --epoch <epoch>`, and `clear --epoch <epoch>`. It explicitly selects the new endpoint, uses JSON metadata and exact byte exports, and never reads/writes the OS clipboard. See [client commands and safety contracts](shelf-client.md).

`shelf-daemon [--socket-path <private-path>]` serves the new item-aware protocol. `session-proxy` bridges it over binary stdio. Add `--legacy-socket-path <private-path>` to the shelf daemon to serve the same store to V2 clients; `--legacy-max-frame-bytes` defaults to 10 MiB. Existing `proxy` clients use that compatibility socket, while new sessions use the separate shelf socket. Startup refuses an occupied socket. This does not install or replace the current server. See [endpoint policy and native evidence](shelf-server.md).

### `server-maintenance` (Linux only)

`verify --version <version> --public-key <trusted.pub> --package <binary> --signature <binary.sig>` authenticates bounded files without executing/installing them.

`install --version <version> --public-key <trusted.pub> --acknowledge-clipboard-loss` creates a separate per-user signed shelf installation and accepts the `shelf-daemon` socket/limit options. `update --version <newer-version> --acknowledge-clipboard-loss` retains pinned trust/options and restores the previous executable after failed service/protocol health. Both accept paired `--package` / `--signature` local inputs; omit both to download the exact GitHub `server-v<version>` asset. Signed server 0.4.0 assets are published; see [live/public acceptance](history/linux-cutover-acceptance.md).

`status` prints saved selection/recovery metadata, without asserting live health. `recover --acknowledge-clipboard-loss` resolves an unfinished transition by restoring the retained predecessor or disabling a failed first installation. These four commands accept `--root <absolute-path>`; default storage is the user's data directory under `ssh_clipboard/server`. Restart/rollback loses in-memory items. Existing global/legacy installs are not adopted. See [maintenance and migration gates](server-maintenance.md).

### `settings`

Edit the versioned configuration shared with the desktop. This command is available without the old agent feature; the existing `config` command still edits the legacy agent file.

```powershell
ssh_clipboard settings path
ssh_clipboard settings show
ssh_clipboard settings set --target clipboard-hub --show-preview false
ssh_clipboard settings add-profile --id second --name "Second server" --target other-hub
ssh_clipboard settings select-profile second
```

`settings set` changes supplied fields only: `--target`, `--name`, `--port`, `--identity-file`, `--clear-port`, `--clear-identity-file`, `--show-preview true|false`, `--close-after-copy true|false`. `--settings-file <path>` selects a separate file for these commands. Corrupt or concurrently modified settings are preserved and reported; Show never creates a default file.

`settings remove-profile <id>` removes a local connection profile. Removing the selected profile selects the first remaining profile in saved order; removing an inactive profile keeps the current selection. Missing or last-profile removal fails without writing. Servers and clipboard contents are untouched.

Advanced Set flags are `--ssh-option` (repeatable, appended), `--clear-ssh-options`, `--ssh-bin`, `--clear-ssh-bin`, `--max-size`, `--timeout-ms`, `--resync-frames true|false`, `--resync-max-bytes`, and `--device-name`. Connection policy must remain valid; a settings-file size/timeout/resync budget of zero is rejected.

`settings migrate-agent [--legacy-file <path>]` initializes only missing shared settings and saves an exact legacy backup. Existing shared settings win. Default desktop startup/target-less CLI discovery can do this automatically; custom files require the explicit command. Schema 1 prototype settings upgrade in memory and get an exact backup before schema 2 is saved. See [migration/recovery](configuration-migration.md) before returning to an older prototype binary.

Push/pull/peek/doctor commands accept `--profile <id>` and `--settings-file <path>`. Without a destination, they use the selected saved profile. An explicit profile allows command-line target/host/port/key overrides. With an explicit destination and no profile flag, they use an independent connection and do not inherit another profile's port/key. The desktop reloads CLI edits approximately every two seconds, deferring them during a transfer. It preserves edited drafts and blocks stale saves/transfers. See [shared configuration](shared-configuration.md) for the complete precedence and recovery rules.

### `push`
Send local clipboard to server (text or PNG).

In a headless `--no-default-features` build, use `--stdin`; OS clipboard requests fail before SSH with exit code 6. Default clipboard CLI and opt-in agent builds retain clipboard support. A clipboard-only CLI can use `--no-default-features --features system-clipboard`.

Common usage:
```
ssh_clipboard push --target user@server
```

Flags:
- `--stdin`: read text from stdin instead of clipboard
- `--target user@host[:port]` (for simple hostnames; use `--port` for IPv6)
- `--host`, `--user`, `--port`
- `--identity-file <path>`
- `--ssh-option <opt>` (repeatable; passed as `ssh -o <opt>`)
- `--ssh-bin <path>`
- `--timeout-ms <ms>`
- `--max-size <bytes>`
- `--strict-frames`: disable framing resync (strict MAGIC at byte 0)
- `--resync-frames true|false`: override a saved resync choice (conflicts with `--strict-frames`)
- `--resync-max-bytes <bytes>`: max bytes to discard before MAGIC (default 8192)

### `pull`
Fetch from server and write to clipboard (default), or output to stdout/file.

Headless builds require `--stdout`/`--output`, or `--peek` for metadata. They do not fetch a payload before reporting missing OS clipboard support.

Common usage:
```
ssh_clipboard pull --target user@server
```

Flags:
- `--stdout`: print text to stdout
- `--output <file>`: write raw payload to file (PNG or text)
- `--base64`: print binary/image as base64 (requires `--stdout`)
- `--peek`: metadata-only (like `peek`)
- `--json`: with `--peek`, print JSON output
- SSH + timeout + size flags (same as `push`)
- `--strict-frames`, `--resync-max-bytes` (same as `push`)

### `peek`
Fetch metadata only (no payload).

Common usage:
```
ssh_clipboard peek --target user@server
```

Flags:
- `--json`: output JSON (default output is human-readable)
- SSH + timeout + size flags (same as `push`)
- `--strict-frames`, `--resync-max-bytes` (same as `push`)

### `doctor`
Run connectivity diagnostics for SSH/proxy/protocol setup.

Common usage:
```
ssh_clipboard doctor --target user@server
```

Checks resolve the selected target and perform one V2 `PeekMeta` exchange through `ssh_clipboard proxy`. The report distinguishes the executable, SSH, proxy, daemon, protocol and metadata layers; failed/unknown layers remain explicit. It does not run remote `true`, `proxy --help`, or `ssh -V`.

Flags:
- `--target user@host[:port]` (for simple hostnames; use `--port` for IPv6)
- `--host`, `--user`, `--port`
- `--identity-file <path>`
- `--ssh-option <opt>` (repeatable; passed as `ssh -o <opt>`)
- `--ssh-bin <path>`
- `--timeout-ms <ms>` (default 7000)

Notes:
- `doctor` uses the same shared settings/migration discovery as transfers, including saved transport policy. It does not use the old default-creating agent loader. Explicit destinations do not inherit profile defaults. Corrupt settings produce an error rather than silently selecting another target. Probes force batch mode so diagnostics remain non-interactive even when an imported option requests prompts.
- Doctor sends only `PeekMeta` through the clipboard proxy, so proxy-only forced-command keys do not need permission for `true` or `--help`. Empty clipboard is healthy. Failed checks return exit status 2, including a missing daemon; later unverified steps are shown as warnings. Reports include static next steps and exclude raw SSH/server output. See [connection diagnostics](connection-diagnostics.md).

## Retired Agent Commands

This section requires `--features agent` or a frozen legacy archive. Use the installed desktop for new Windows/Apple Silicon setups; [legacy agent guidance](legacy/agent.md) owns startup and Linux platform details.

### `agent` (Windows/macOS/Linux)
Run the background agent (tray icon + hotkeys).

Flags:
- `--no-tray`: disable tray UI
- `--no-hotkeys`: disable hotkeys

Notes:
- `ssh_clipboard agent` runs in-process when attached to a terminal; otherwise it launches the `ssh_clipboard_agent` binary (matching autostart behavior).
- On Linux Wayland sessions, hotkeys are auto-disabled at startup with a single notice; use tray actions, or run under X11 to enable global hotkeys.
- `--no-hotkeys` remains available as a manual override on all platforms.

### `install-client` (Windows/macOS/Linux)
Install client binaries to a stable user-local location, update PATH, run setup, verify, and start the agent.

Common usage:
```
# from extracted release folder
./ssh_clipboard install-client --target user@server
```

Flags:
- `--target <user@host>` (required)
- setup/connectivity flags: `--port`, `--identity-file`, `--ssh-option`, `--clear-ssh-options`, `--max-size`, `--timeout-ms`, `--resync-frames`, `--resync-max-bytes`
- `--install-dir <path>`: override default install directory
- `--no-path-update`: skip PATH persistence changes
- `--no-start-now`: skip immediate agent launch after install
- `--dry-run`: print planned actions only
- `--force`: overwrite existing binaries in the install directory

Defaults:
- install directory:
  - Windows: `%LOCALAPPDATA%\\ssh_clipboard\\bin`
  - macOS/Linux: `~/.local/bin`
- verifies autostart and runs `doctor` after setup
- remote verification failures are warnings (local install still succeeds)

### `uninstall-client` (Windows/macOS/Linux)
Remove binaries and PATH/autostart integration created by `install-client`.

Common usage:
```
ssh_clipboard uninstall-client
```

Flags:
- `--install-dir <path>`: override default install directory
- `--no-path-cleanup`: skip PATH cleanup
- `--dry-run`: print planned actions only
- `--force`: continue on non-critical cleanup errors

Notes:
- Keeps agent config/log files by default.
- On Windows, uninstall attempts to stop running `ssh_clipboard_agent.exe` processes before removing binaries.
- If a Windows binary is still in use, removal is best-effort deferred.

### `setup-agent` (Windows/macOS/Linux)
One-command setup for the agent: writes config (sets target) and enables autostart.

Common usage:
```
ssh_clipboard setup-agent --target user@server
```

Flags:
- `--no-autostart`: do not enable autostart (disables if already enabled)
- `--dry-run`: print the resulting config and planned actions without changing the system
- accepts the same connection options as `config set` (port/identity/ssh-option/etc.)

### `config`
Manage agent configuration.

Subcommands:
- `config path`
- `config show [--json]`
- `config validate`
- `config defaults`
- `config set --target user@host [--port 2222] [--identity-file <path>] [--ssh-option <opt>] [--clear-ssh-options] [--max-size <bytes>] [--timeout-ms <ms>] [--resync-frames <bool>] [--resync-max-bytes <bytes>]`

### `autostart`
Manage “start at login” for the agent.

Subcommands:
- `autostart enable`
- `autostart disable`
- `autostart status`
- `autostart refresh`

## V2 Server Compatibility Commands

These commands run or install the standalone single-item server. New server installations use [managed shelf maintenance](server-maintenance.md).

### `daemon` (Linux only)
Run the per-user daemon that stores clipboard contents in memory.

Flags:
- `--socket-path <path>`
- `--max-size <bytes>`
- `--io-timeout-ms <ms>`

### `proxy` (Linux only)
Run the proxy (invoked over SSH).

Flags:
- `--socket-path <path>`
- `--max-size <bytes>`
- `--io-timeout-ms <ms>`
- `--autostart-daemon`: attempt to start the daemon if the socket is unavailable

### `install-daemon` (Linux only)
Set up the daemon and systemd user service in one command.

Common usage:
```
./ssh_clipboard install-daemon
```

Flags:
- `--dry-run`: print actions and unit contents without changing the system
- `--force`: overwrite existing unit source/link
- `--no-sudo`: do not use sudo (fails if `/usr/local/bin` cannot be updated)
- `--max-size <bytes>`
- `--io-timeout-ms <ms>`
- `--socket-path <path>`

### `uninstall-daemon` (Linux only)
Remove the systemd user service and PATH symlink created by `install-daemon`.

Common usage:
```
./ssh_clipboard uninstall-daemon
```

Flags:
- `--dry-run`: print actions without changing the system
- `--no-sudo`: do not use sudo (fails if `/usr/local/bin` cannot be removed)

## Exit Codes (client)
- `0`: success
- `2`: invalid request/response or unsupported content
- `3`: payload too large
- `4`: daemon not running / socket unavailable
- `5`: SSH failure
- `6`: clipboard read/write failure

## Update Triggers

- Commands, feature gates, flags, defaults, output formats or exit status change.
- Endpoint selection or configuration precedence changes.

## Related Docs
- [docs/legacy/client-setup.md](legacy/client-setup.md)
- [docs/legacy/agent.md](legacy/agent.md#linux-platform-notes)
- [docs/server-setup.md](server-setup.md)
- [docs/protocol.md](protocol.md)
