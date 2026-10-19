# ssh_clipboard

`ssh_clipboard` shares text and PNG clipboard content between machines over SSH. Sharing and copying are explicit; the app never auto-pastes into another application.

- Desktop packages: Windows x64 and Apple Silicon macOS.
- Server package: Linux x86_64, with bounded temporary history held in memory only.
- CLI: clipboard transfers, explicit stdin/file exports, shared settings and metadata diagnostics.

## Install the desktop

[Download desktop 0.4.2](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.4.2). Both packages include the desktop and matching CLI, with signed package verification for updates.

1. On Windows, run the per-user installer. On Apple Silicon Mac, extract the `.app.tar.gz` and move **SSH Clipboard.app** into a stable Applications folder before opening it.
2. In Settings, enter your SSH alias or `user@host`, Apply and test the saved connection. Select **Single shared item (legacy server)** for an older server, or **Temporary history (new server)** for a configured shelf server. This choice does not install a server.
3. Use **Share current clipboard** to upload and **Copy to this computer** to replace the local clipboard. Selecting or previewing history does not copy it. Panel Copy uses the selected history item, or the displayed snapshot on a legacy endpoint.
4. Optionally enable **Start at login** in Behavior and configure shortcuts. Quit the old agent before enabling conflicting startup or shortcuts; installation does not remove it automatically.

Close hides the panel in the tray/menu bar; Quit exits. Updates offers explicit download/verification and a separate install confirmation. The Mac app is ad-hoc signed without Apple notarization; use macOS per-app Privacy & Security approval if needed. See [first-install trust](docs/security.md#signed-package-trust). Intel Mac and Linux desktop have no supported new package in this delivery.

## Settings and everyday behavior

Current source uses one Apply/Discard draft across all settings sections, including Start at login. Back, Escape and hiding retain edits for the current app session; transfers continue to use saved settings. Published packages reflect their recorded source and may precede these UI improvements. See [desktop development](docs/desktop-development.md) for source builds and [release evidence](docs/history/release-readiness.md) for exact package boundaries.

Current Windows source also starts SSH in the background without opening a console window. See [SSH session ownership](docs/persistent-sessions.md#ownership-and-reuse).

Named connection profiles retain SSH aliases, port/key references and advanced policy. The desktop reloads CLI edits while protecting unsaved drafts. `ssh_clipboard settings` inspects shared settings and edits connection/transport/basic preferences; shortcut controls are in the desktop. Legacy settings import creates exact recovery backups. See [shared settings](docs/shared-configuration.md) and [migration/recovery](docs/configuration-migration.md).

Shelf profiles update history metadata automatically and recover transient connections with bounded backoff. Trust/authentication errors pause for explicit action. Metadata refresh/reconnect does not fetch clipboard payloads or change the local clipboard. Active operations expose Cancel; an uncertain Share can be checked or explicitly retried from its original capture within a server-enforced window. See [history](docs/desktop-history.md) and [share recovery](docs/share-recovery.md).

Legacy transfers default to a 10 MiB frame limit including metadata, with a separate 64 MiB decoded-image budget. Shelf defaults negotiate up to 64 MiB encoded and 256 MiB decoded per item, narrowed by client/server policy. Rejected shares preserve existing server content. Raising a client setting does not reconfigure a server.

## Linux server

Use the [signed server 0.4.0 release](https://github.com/johansigfrids/ssh_clipboard/releases/tag/server-v0.4.0) and [managed server setup](docs/server-setup.md). Independently verify first-install trust before using `server-maintenance install|update|status|recover|verify`. The managed installer retains immutable executables, a stable per-user launcher/unit, pinned trust and failed-health rollback.

The shelf provides temporary text/PNG history and an optional V2 compatibility socket for older clients. The session and V2 SSH routes are separate. Existing services and forced-command keys require a reviewed cutover; no profile edit adopts or replaces them. Restart/rollback loses in-memory items. See [compatibility and cutover](docs/compatibility-and-cutover.md).

## CLI usage

These commands use the V2 latest-item endpoint:

```text
ssh_clipboard push --target user@server
ssh_clipboard pull --target user@server
ssh_clipboard peek --target user@server
ssh_clipboard doctor --target user@server
```

Explicit input/output avoids OS clipboard access:

```sh
cat note.txt | ssh_clipboard push --stdin --target user@server
ssh_clipboard pull --stdout --target user@server
ssh_clipboard pull --output ./clipboard.png --target user@server
```

The `shelf` namespace separately lists history, shares explicit file/stdin content and exports selected items. See [shelf commands](docs/shelf-client.md) and the [CLI reference](docs/cli.md). Use `ssh_clipboard <command> --help` for flags.

## Build from source

The default build includes the clipboard CLI. A Linux server/stdio-only build excludes native clipboard and desktop dependencies:

```text
cargo build --release
cargo build --release --no-default-features
```

For the desktop, use Node.js 24, Rust and [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/), then from `desktop/`:

```powershell
npm ci
npm run tauri dev
```

Standalone development artifacts, settings isolation and resource guardrails are in [desktop development](docs/desktop-development.md). [Testing](docs/testing.md) lists Rust, frontend, package and documentation checks.

## Legacy compatibility and support limits

The old tray agent is retired from default builds and automatic releases. Its [explicit compatibility build](docs/legacy/agent.md) remains available through 0.4:

```text
cargo build --release --features agent
```

Frozen [v0.3.0 archives](https://github.com/johansigfrids/ssh_clipboard/releases/tag/v0.3.0), V2 routes and legacy import remain available under the [retirement policy](docs/legacy-retirement.md). Linux desktop is deferred. Native Mac image paste, browser-download Gatekeeper behavior and actual server reboot/logout remain recorded evidence limits; see the [closed delivery scope](docs/history/implementation-plan.md#deferred-and-non-blocking).

## Documentation

Start at the [docs index](docs/index.md) for current development, runtime and operations guides, legacy compatibility and dated acceptance history. Use [troubleshooting](docs/troubleshooting.md) for failures.

## License

MIT. See [LICENSE](LICENSE).

## Disclaimer

This project is 100% vibecoded and has not had human code review.
