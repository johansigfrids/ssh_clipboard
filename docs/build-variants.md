# Build Variants and Dependency Boundaries

## Purpose

Keep server and stdio client builds independent of OS clipboard, tray, hotkey, and desktop libraries while sharing protocol, image, transport, configuration, and transfer logic.

## Key Files

- `Cargo.toml`: optional `system-clipboard` and legacy `agent` features; PNG-only image codec.
- `src/client/image.rs`: shared RGBA pixels and bounded PNG codec, without OS types.
- `src/client/clipboard.rs`: optional native clipboard adapter.
- `src/client_actions.rs`: adapter trait and reusable transfer actions.
- `desktop/src-tauri/src/clipboard.rs`: desktop-owned retained adapter.
- `.github/workflows/ci.yml`: headless dependency guard and platform builds.

## Variants

| Build | Command | Behavior |
| --- | --- | --- |
| Retired agent compatibility | `cargo build --features agent` | Explicit opt-in only; keeps old tray/hotkeys/install/config commands available through 0.4. No new automatic agent releases. |
| Default clipboard CLI | `cargo build` | OS clipboard transfers/settings without the retired tray/agent or its setup commands. Also selectable explicitly with `--no-default-features --features system-clipboard`. |
| Headless server/stdio CLI | `cargo build --no-default-features` | Linux daemon/proxy, shared settings, SSH transport, stdin/stdout/file transfers; no OS clipboard or desktop libraries. |
| New desktop | `desktop/` source/build commands | Owns its native clipboard adapter and links the root library with default features disabled. |

The headless build requires `push --stdin` and `pull --stdout`/`--output` for payload transfers. `peek`, `pull --peek`, and settings remain available. Unsupported OS clipboard requests fail with exit code 6 and an explanation before SSH starts. An invalid profile still fails during configuration resolution first.

The root codec supports PNG only, the protocol's image representation. Enabling every default image codec had pulled unrelated AVIF/JPEG/etc. dependencies into the server. GUI icon decoding also uses PNG. Add other formats only with a representation/protocol requirement and explicit resource tests.

## Guardrails and Evidence

The old shared pixel type was `arboard::ImageData`, so building the server also compiled shared image/action APIs against an OS clipboard library. Use the owned/borrowed RGBA type in `src/client/image.rs` across shared code; convert at OS adapters. The retained standalone daemon checks content type, encoded length and text UTF-8; the delivered shelf validates PNG bytes and decoded budgets before commit. The desktop must retain its adapter on its worker thread rather than delegating ownership to one-shot CLI helpers.

The default build tree also rejects old agent/tray/startup dependencies, and the executable help contract checks that normal transfers/settings remain without agent/setup/autostart commands. See [retirement](legacy-retirement.md).

CI checks the headless runtime tree for clipboard, tray, hotkey, X11/Wayland, GTK, and registry integration libraries. The Linux server job does not install agent GUI prerequisites. Keep optional dependencies out of normal imports and test types; adapter contract tests use the shared pixel type so they also run headlessly.

At the initial dependency-split checkpoint on 2026-10-14, Windows headless (42 unit + 5 executable checks), legacy agent (56 + 4), and desktop backend (18) tests passed, with required lint/format checks. Subsequent migration adds its own probes; see [migration evidence](configuration-migration.md#evidence). The Linux-target normal dependency tree remains free of OS clipboard/desktop libraries after those additions. This is dependency evidence, not a native Linux build: the authorized test server has no Cargo available in its SSH environment, and no toolchain was installed. CI's Linux compilation/native platform checks remain required. No running server was replaced or restarted.

Subsequent 2026-10-14 evidence closes the headless Linux compilation gap: the existing Ubuntu WSL Rust 1.97.1 toolchain runs native unit, migration, settings/doctor, and real shelf/legacy daemon-proxy process probes without GUI libraries. Output uses `target/linux-wsl` to avoid mixing Windows artifacts. See [native evidence and limits](shelf-server.md#evidence-and-remaining-work). This does not establish a Linux desktop GUI session or a remote-host rollout.

Mac packaging has a separate CI probe: `npm run test:package -- --release` builds installed features with a disposable updater key and test app identifier, then inspects the archived app/CLI and their code signatures without launching or installing the GUI. It needs no production signing secrets or build tools on the owner's Mac. See [Mac cutover](macos-cutover.md) for scope and pending native gates.

## Update Triggers

- A shared type/module gains an OS dependency.
- Build features, supported formats, platform prerequisites, or CI variants change.
- Native Linux compilation or adapter evidence becomes available.

## Related Docs

- [Architecture](../ARCHITECTURE.md), [implementation plan](implementation-plan.md)
- [Testing](testing.md), [CI](ci.md), [dependency decisions](dependency-decisions.md)
- [Desktop prototype](desktop-prototype.md), [server setup](server-setup.md)
