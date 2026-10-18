# ssh_clipboard

`ssh_clipboard` copies clipboard content between machines over SSH.

- Desktop: Windows x64 / Apple Silicon macOS (Linux desktop is deferred)
- Server: Linux (daemon + proxy)

The Linux shelf server keeps bounded temporary clipboard history **in memory only** (no on-disk persistence). Its optional V2 compatibility endpoint exposes the latest item to older clients.

## Windows and Apple Silicon desktop

[Download SSH Clipboard 0.4.2](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.4.2) for Windows x64 or Apple Silicon Mac. Both packages include the desktop app and matching CLI, with signed package verification for updates.

1. On Windows, run the per-user installer. On Apple Silicon Mac, extract the `.app.tar.gz` archive and move **SSH Clipboard.app** into a stable Applications folder before opening it.
2. In Settings, enter your SSH alias or `user@host`, save and test the connection. Existing Linux servers use **Single shared item (legacy server)** under Server features; history requires a separately installed shelf server.
3. Use **Share current clipboard** to upload and **Copy to this computer** to replace the local clipboard with the shared item. Sharing and copying happen only when requested.
4. Optionally enable **Start at login** in Behavior. Quit the old agent before enabling conflicting shortcuts/startup; installation does not remove it automatically.

Close hides the panel in the tray/menu bar; Quit exits. Updates are available in Settings with explicit download/verification and install confirmation. The Mac app is ad-hoc signed without Apple notarization; if first launch needs approval, use macOS per-app Privacy & Security controls. See [first-install public trust](docs/security.md#signed-package-trust). Intel Mac and Linux desktop have no new supported package in this delivery. Frozen [v0.3.0 archives](https://github.com/johansigfrids/ssh_clipboard/releases/tag/v0.3.0) remain available for compatibility.

### Build the desktop from source

From `desktop/`, with Node.js 24, Rust, and [Tauri platform prerequisites](https://v2.tauri.app/start/prerequisites/):

```powershell
npm ci
npm run tauri dev
```

Enter your SSH alias or `user@host` in Settings, apply it, test the connection, then refresh or share. Copy writes the displayed item to your local clipboard. Close hides the panel; Quit exits the app.

Apply settings and Discard changes stay visible in every settings section, including Updates. Unsaved changes take effect only after Apply; Back, Escape, and hiding the panel keep your draft for this app session. The panel shows an unsaved-settings notice with Return to settings and Discard changes. Transfers continue using saved settings. Preview visibility and Start at login also wait for Apply. Start at login shows the actual OS state until edited; saving unrelated preferences leaves that startup entry unchanged. If startup state is unavailable, you can still save other settings.

Updates shows installed and available versions with Download update before What's new. Downloaded updates are verified before installation, and installing/restarting requires confirmation. Short version-specific notes appear as a list; longer or older notes can be expanded without hiding the update actions beneath them.

Legacy profiles default to a 10 MiB transfer limit, including request metadata, and honor the selected profile's limit. Images use lossless PNG compression with a separate 64 MiB pixel limit. Shelf profiles negotiate their limits with the server. Size errors explain what exceeded the limit; rejected shares keep the previous item. A client override does not reconfigure the server.

Desktop settings are stored separately from the old agent. Connection settings support named profiles: add, rename, choose and Apply, or remove with confirmation. A collapsed Advanced section edits per-profile SSH options, executable, timeout, transfer limit, and framing recovery. Test connection shows which steps passed or failed and what to check next. Optional direct Share/Copy shortcuts have recorders and explicit opt-in; see [shortcut behavior](docs/desktop-shortcuts.md). The rework CLI can inspect/edit them through `ssh_clipboard settings`; transfers without an explicit destination use the selected profile. The desktop reloads CLI edits automatically and protects unsaved drafts. Legacy/prototype settings migrate with exact backups; old startup registrations remain untouched. A one-time review explains imported connection/device, shortcut and startup choices; dismissing it preserves the original settings and recovery backup. Behavior settings choose panel, private notification, or quiet feedback for direct actions. Portable Windows builds use panel fallback for requested notifications. Shelf profiles now update history metadata automatically and recover transient network failures with bounded backoff. Authentication/trust errors pause with an explicit Reconnect history action; recovery never shares or copies automatically. Active operations expose Cancel. An uncertain shelf share can be checked or explicitly retried from its original capture within a server-enforced window; recovery never reads a newer clipboard. See [share recovery](docs/share-recovery.md). Installed-build staging now bundles a matching CLI in a signed per-user package. Release builds expose explicit start-at-login controls; portable prototypes cannot register startup. The panel is created hidden and unfocused; normal launches reveal it, while background startup stays in the tray. If the tray is unavailable, the panel opens so the app remains reachable. An isolated Windows test-app installation and signed manual upgrade have passed, including retained connection/startup registration and background launch without a panel flash; actual Windows sign-in also passed. Private Windows updater tests passed for same-key replacement, cancellation, tampered-package rejection and an installed signing-key transition. Installed builds also have bounded quiet update checks, explicit verified downloads, and guarded install confirmation. Rechecking keeps the release card visible and shows progress within Updates. Protected production signing and anonymous Windows/Apple Silicon public delivery pass. Clean Mac migration, text transfer, login/shortcut and isolated native update/restart acceptance also pass; native Mac image paste, browser-download Gatekeeper behavior and Linux desktop installation remain unverified; see [package/startup evidence](docs/desktop-updates.md). See the [prototype guide](docs/desktop-prototype.md), [shared settings](docs/shared-configuration.md), [migration/recovery](docs/configuration-migration.md), and [implementation plan](docs/implementation-plan.md).

The Linux server includes a bounded shelf and session proxy. An optional V2 compatibility socket lets existing clients use its latest item while new sessions access history. The explicit `shelf` CLI now lists metadata, shares file/stdin content, exports selected items, and guards delete/clear by server epoch; see [client commands](docs/shelf-client.md). Connection settings can now opt into [desktop history](docs/desktop-history.md), including selection, safe copy, and confirmed delete/clear. Existing profiles keep V2 transfers. The server requires an explicit cutover; see [endpoint commands and compatibility](docs/shelf-server.md).

The [Linux x86_64 server 0.4.0 release](https://github.com/johansigfrids/ssh_clipboard/releases/tag/server-v0.4.0) provides signed maintenance through `server-maintenance install|update|status|recover|verify`: stable user-owned executables, pinned public trust, and rollback after failed protocol health. The commands do not adopt the legacy service automatically. Disposable signed-binary and real user-systemd runtime tests pass, including crash-loop rollback and cleanup. The signed server has passed a [clean live installation and anonymous public-delivery verification](docs/linux-cutover-acceptance.md), with persistent unit/SSH health and retirement of the old installation. Actual reboot/last-session logout remain separate. See [managed server setup](docs/server-maintenance.md). Actual published Linux v0.3.0 CLI/proxy/daemon compatibility is tested on disposable sockets for both protocol upgrade orders; see [tested pairings and limits](docs/released-compatibility.md).

Windows and Apple Silicon desktop **0.4.2** and Linux server **0.4.0** are published; the clean live Linux migration is accepted. [Release readiness](docs/release-readiness.md) records installed/public-delivery evidence. The [implementation plan](docs/implementation-plan.md) is closed for this scope; Linux desktop and omitted rare manual checks are deferred and non-blocking. V2 routes and explicit compatibility builds remain available throughout the 0.4 series.

## Retired legacy agent

The signed desktop replaces the old tray agent. New source builds and automatic releases no longer include that agent by default. Its source is retained through 0.4 as an explicit compatibility build:

```powershell
cargo build --release --features agent
```

Use the current [managed server setup](docs/server-maintenance.md) for Linux installations. Frozen v0.3.0 downloads and the [old agent](docs/agent.md)/[client setup](docs/client-setup.md) references remain available for existing installations; they are not the recommended first-time setup. Legacy settings import and V2 CLI/server compatibility remain supported. See [the retirement boundary](docs/legacy-retirement.md).

## Essentials (daily usage)

### CLI (ad hoc / scripts)
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

### Input/output modes
Push text from stdin:
```
cat note.txt | ssh_clipboard push --stdin --target user@server
```

Pull to stdout (instead of clipboard):
```
ssh_clipboard pull --stdout --target user@server
```

Pull PNG to a file:
```
ssh_clipboard pull --output ./clipboard.png --target user@server
```

### Robust defaults
- The client tolerates “noisy shells” / MOTD bytes before the protocol by resyncing frames by default.
  - Use `--strict-frames` to disable resync and fail fast instead.
- Default max payload is **10 MiB** (`--max-size`).

## Feature overview
- Clipboard formats: UTF-8 text and PNG images
- Desktop: visible Share/Copy, temporary history, settings, optional shortcuts and login startup
- Shelf sessions: live metadata and explicit transfer/recovery; V2 CLI routes retain one-shot requests
- Verified desktop updates and signed Linux server maintenance
- Server: per-user daemon, UNIX socket permissions, peer credential checks (Linux)

## More help (user docs)
This repo’s `docs/` folder is intended for internal/dev documentation.

For user-facing help, rely on CLI help:
```
ssh_clipboard --help
ssh_clipboard <command> --help
```

Useful starting points:
- `ssh_clipboard doctor --target user@server`
- `ssh_clipboard settings --help`
- `ssh_clipboard shelf --help`
- `ssh_clipboard server-maintenance --help`
- `ssh_clipboard push --help`
- `ssh_clipboard pull --help`
- `ssh_clipboard peek --help`

## Developer docs (internal)
If you’re hacking on the project or packaging it, start at `docs/index.md`.

## Build from source
Clipboard CLI build (without the retired agent):
```
cargo build --release
```

Linux server-only build (no agent/UI deps):
```
cargo build --release --no-default-features
```

## License
MIT. See `LICENSE`.

## Disclaimer
This project is 100% vibecoded and has not had human code review.
