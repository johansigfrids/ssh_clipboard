# ssh_clipboard

`ssh_clipboard` shares text, PNG clipboard content, and files/folders between machines over SSH. Sharing and copying are explicit; the app never auto-pastes into another application.

File transfers describe current source, require upgraded clients/server (session version 2), and are not included in the published 0.5.0 packages linked below. Existing settings are preserved.

Your computers connect to a Linux server running `ssh_clipboard` through your existing SSH aliases, keys and host verification. Share an item from one computer, then select and copy it on another.

- Desktop packages: Windows x64 and Apple Silicon macOS.
- Server package: Linux x86_64, with bounded temporary history held in memory only.
- CLI: explicit session file/stdin/stdout transfers, shared settings and metadata Health.

Version 0.5.0 requires fresh settings and managed-server setup. See [cutover](docs/compatibility-and-cutover.md) before installing it; older published packages retain their recorded behavior.

![SSH Clipboard desktop panel showing a selected text item, Share and Copy actions, and temporary history](docs/images/desktop-ux-pr6/history-after.png)

*Development screenshot; published packages may differ.*

## Set up the Linux server

Before connecting a desktop, you need a Linux x86_64 server account with SSH access and a functioning user systemd manager. The desktop does not install the server for you.

Follow [managed server setup](docs/server-setup.md) for the exact reviewed server version. Use the signed [server 0.5.0 package](https://github.com/johansigfrids/ssh_clipboard/releases/tag/server-v0.5.0) for fresh setup. That guide covers first-install signature trust, installation, the SSH command route and a connection check. Use the explicit server release; desktop and server versions are released separately.

History is held in memory, expires automatically and is lost when the server restarts.

For 0.5, follow [fresh setup and cutover](docs/compatibility-and-cutover.md). Only the session endpoint is supported.

## Install the desktop

[Download desktop 0.5.0](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.5.0). Both packages include the desktop and matching CLI, with signed package verification for updates.

1. On Windows, run the per-user installer. On Apple Silicon Mac, extract the `.app.tar.gz` and move **SSH Clipboard.app** into a stable Applications folder before opening it.
2. In Settings, enter your server's SSH alias or `user@host`. Apply and test the saved connection.
3. Use **Share current clipboard** to upload. On another computer connected to the same server account, select the item and choose **Copy to this computer**, then paste it into your application. Selecting or previewing history does not copy it.
4. Optionally enable **Start at login** in Behavior and configure shortcuts. Review startup ownership when replacing an existing installation.

With file-capable builds, copy files/folders in Explorer or Finder, Share them, Copy on the receiving computer, then paste into its file manager. Selections default to a 64 MiB limit. Received files use a retained local disk cache; incompatible filenames are renamed. See [file transfers](docs/file-transfers.md) for limits, privacy, cleanup and metadata exclusions.

The Mac app is ad-hoc signed without Apple notarization; use macOS per-app Privacy & Security approval if needed. See [first-install trust](docs/security.md#signed-package-trust). Intel Mac and Linux desktop have no supported package in this delivery.

## Settings and everyday behavior

Close hides the panel in the tray/menu bar; Quit exits. Configure panel and transfer shortcuts in Settings. Source builds default to Ctrl+Alt+Shift+S (Cmd+Option+Shift+S on Mac) to open the panel; existing saved bindings are preserved. See [shortcuts](docs/desktop-shortcuts.md) to change or restore the default. Updates offers explicit download/verification and a separate install confirmation.

Save connection settings before transferring; transfers use the saved configuration. Named profiles let you switch between servers, and the desktop and CLI share settings. See [shared settings](docs/shared-configuration.md) and [fresh setup](docs/compatibility-and-cutover.md).

Temporary history refreshes automatically and selects the newest item when it arrives, ready for Copy. You can still select an older item; ordinary refreshes retain that selection until another new item arrives. These updates do not read or replace your local clipboard. Panel Copy fetches the selected history item. See [history controls](docs/desktop-history.md).

For connection failures or an interrupted Share, start with [troubleshooting](docs/troubleshooting.md) and [share recovery](docs/share-recovery.md). Transfer limits and reconnect details are in the [shelf client](docs/shelf-client.md) and [SSH sessions](docs/persistent-sessions.md) guides.

## CLI usage

Use `shelf` commands for the temporary-history server. After saving a connection in desktop Settings, these commands use the selected profile and this computer's saved device identity:

```text
ssh_clipboard shelf health
ssh_clipboard shelf share --file note.txt
ssh_clipboard shelf list
ssh_clipboard shelf fetch <item-uuid> --output received-note.txt
ssh_clipboard shelf share-files report.pdf photos
ssh_clipboard shelf fetch-files <file-item-uuid> --output-dir received
```

Replace `<item-uuid>` with an item's UUID from `list`. These commands read or write explicit files and leave the OS clipboard alone; an existing output file is never overwritten. Use `--stdin` instead of `--file` to share piped text, or add `--format png` when sharing a PNG file.

`share-files` captures files/folders as a bundle; `fetch-files` extracts to a new directory and prints final paths/renames as JSON. Existing `fetch` exports raw bundle bytes for file items.

For CLI-only connection setup, see [shared settings](docs/shared-configuration.md#cli-selection-and-edits). See [shelf commands](docs/shelf-client.md) for direct targets, profile selection and more examples, or the [CLI reference](docs/cli.md) for all commands. Use `ssh_clipboard <command> --help` for flags.

## Support boundary

The 0.5 source removes the old tray agent, V2 mode, old installers and native clipboard CLI commands. Old settings are not imported. Use [fresh setup](docs/compatibility-and-cutover.md); published packages and prior acceptance remain in [history](docs/index.md#history-and-acceptance). Linux desktop remains deferred.

## Build from source

The root build is the headless session CLI and Linux server, without native clipboard or agent dependencies:

```text
cargo build --release
```

For the desktop, use Node.js 24, Rust and [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/), then from `desktop/`:

```powershell
npm ci
npm run tauri dev
```

Standalone development artifacts, settings isolation and resource guardrails are in [desktop development](docs/desktop-development.md). [Testing](docs/testing.md) lists Rust, frontend, package and documentation checks.

Source builds include UI improvements that may not be in published packages. The feature guides describe current source; [release evidence](docs/history/release-readiness.md) records exact package boundaries.

## Documentation

Start at the [docs index](docs/index.md) for current development, runtime and operations guides, fresh cutover guidance and dated acceptance history. Use [troubleshooting](docs/troubleshooting.md) for failures.

## License

MIT. See [LICENSE](LICENSE).

## Disclaimer

This project is 100% vibecoded and has not had human code review.
