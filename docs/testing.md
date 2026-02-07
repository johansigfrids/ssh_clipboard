# Testing

## Purpose
Describe how to run tests locally and what kinds of tests exist in this repo.

## Key Files
- `src/framing.rs` (framing unit tests + proptest)
- `src/protocol.rs` (protocol round-trip tests)
- `src/client_actions.rs` (clipboard contract tests)
- `src/daemon.rs` (Linux-only daemon tests)
- `Cargo.toml` (dev-dependencies for tests)

## How to Run
Basic unit tests (agent is enabled by default):
```
cargo test
```

CI (runs on GitHub Actions, 4 build variants):
```
cargo fmt -- --check
cargo clippy --features agent -- -D warnings
cargo test --features agent
cargo build --release --features agent
```

With agent feature:
```
cargo test --features agent
```

## Linux Build Dependencies (agent)
Ubuntu packages needed to build the agent on Linux:
```
sudo apt-get update
sudo apt-get install -y \
  pkg-config \
  libglib2.0-dev \
  libgtk-3-dev \
  libappindicator3-dev \
  libdbus-1-dev \
  libxdo-dev \
  libx11-dev \
  libxkbcommon-dev \
  libwayland-dev \
  libxrandr-dev \
  libxinerama-dev \
  libxcursor-dev \
  libxi-dev \
  libxfixes-dev \
  libxcb1-dev \
  libxcb-shape0-dev \
  libxcb-xfixes0-dev
```

## Test Categories
- **Framing/protocol**: round-trip, invalid framing, truncated payloads.
- **Client actions**: behavior parity tests using mock clipboard.
- **Image handling**: PNG encode/decode and size guard tests.
- **Daemon (Linux-only)**: socket path permissions, request validation, read timeouts.

## Notes
- Some tests are Linux-only (`#[cfg(test)]` in `src/daemon.rs`).
- Property tests use `proptest`; keep bounds small to avoid long test runs.
- CI runs four variants: Windows agent, macOS agent, Linux agent, and Linux server (`--no-default-features`).
- CI installs Linux GUI deps so the agent feature can build.

## Platform `cfg` Hygiene
- Keep function/item `#[cfg(...)]` aligned with where it is used.
- If a helper is only called from Windows-gated code, gate the helper too (for example `#[cfg(target_os = "windows")]`).
- If tests need a platform-gated helper on other platforms, use `#[cfg(any(target_os = "windows", test))]`.
- For parameters only used on one platform branch, mark intentional non-use in the other branch (for example `#[cfg(not(target_os = "windows"))] let _ = install_dir;`).
- Before pushing, run the CI lint command locally:
```
cargo clippy --features agent -- -D warnings
```

## Update Triggers
- New features or protocol changes.
- Changes to clipboard handling or agent behavior.
- Adding/removing dev-dependencies for tests.

## Related Docs
- `docs/troubleshooting.md`
- `docs/cli.md`
