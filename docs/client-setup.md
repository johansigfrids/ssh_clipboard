# Client Setup (Windows/macOS)

## Purpose
Document how to run the Phase 2 client CLI on Windows and macOS, including SSH prerequisites and common troubleshooting.

## Key Files
- `README.md`
- `src/main.rs`
- `src/client/ssh.rs`
- `src/client/clipboard.rs`

## Prerequisites
- The Linux server is running `ssh_clipboard daemon`.
- The Linux server user can run `ssh_clipboard proxy` (binary is on `PATH` or invoked via an absolute path).
- The client machine has a working `ssh` binary on `PATH`.
  - Windows: typically “OpenSSH Client” (built-in on recent Windows versions).
  - macOS: OpenSSH is included.

## Basic Usage
Push clipboard to server:
```
ssh_clipboard push --target user@server
```

Pull clipboard from server:
```
ssh_clipboard pull --target user@server
```

Peek metadata:
```
ssh_clipboard peek --target user@server
```

Peek metadata as JSON:
```
ssh_clipboard peek --json --target user@server
```

## Agent Mode (Hotkeys + tray)
The background agent is available when built with the `agent` Cargo feature:
```
cargo run --features agent -- agent
```

The agent uses the config file shown by `ssh_clipboard config path`.

## Input/Output Modes
- `push --stdin`: read text from stdin instead of the clipboard
  - Example: `cat note.txt | ssh_clipboard push --stdin --target user@server`
- `pull --stdout`: print to stdout instead of writing to the clipboard
  - Example: `ssh_clipboard pull --stdout --target user@server`
- `pull --output <file>`: write the raw payload to a file (PNG or text)
  - Example: `ssh_clipboard pull --output ./clipboard.png --target user@server`
- `pull --stdout --base64`: print binary/image data as base64 (explicit opt-in)
  - Example: `ssh_clipboard pull --stdout --base64 --target user@server`
- `pull --peek`: run `PeekMeta` and print metadata without fetching the full payload

## Clipboard Formats (Phase 3)
- Text: `text/plain; charset=utf-8`
- Images: `image/png` (PNG bytes)
- If the server holds an image and the client cannot write images, the client will error unless `--output` or `--base64` is used.

## SSH Configuration
### Target strings
- Preferred: `--target user@host`
- Port:
  - `--port 2222` (recommended)
  - `--target user@host:2222` may work for simple hostnames (not recommended for IPv6/bracketed hosts)

### Passing SSH options
Use `--ssh-option` repeatedly to pass `-o` options through to SSH:
```
ssh_clipboard push --target user@server --ssh-option "ConnectTimeout=5" --ssh-option "ServerAliveInterval=10"
```

## Troubleshooting
- **SSH errors (host key / auth / network):** run the equivalent SSH command manually:
  - `ssh -T user@server ssh_clipboard proxy`
  - Fix SSH issues first (keys, known_hosts, config).
- **Clipboard read/write fails:**
  - Windows: clipboard can be locked by another process; retry.
  - macOS: clipboard access should work for CLI tools, but some environments may require extra permissions.
- **No value set:** `pull` will fail if the server daemon has not received a `push` yet.

## Update Triggers
- Adding richer clipboard formats (images) or changing stdin/stdout behavior.
- Changes to SSH argument handling (target parsing, options, defaults).

## Related Docs
- `docs/server-setup.md`
- `docs/protocol.md`
- `ARCHITECTURE.md`
