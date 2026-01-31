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
Basic unit tests:
```
cargo test
```

With agent feature:
```
cargo test --features agent
```

## Test Categories
- **Framing/protocol**: round-trip, invalid framing, truncated payloads.
- **Client actions**: behavior parity tests using mock clipboard.
- **Image handling**: PNG encode/decode and size guard tests.
- **Daemon (Linux-only)**: socket path permissions, request validation, read timeouts.

## Notes
- Some tests are Linux-only (`#[cfg(test)]` in `src/daemon.rs`).
- Property tests use `proptest`; keep bounds small to avoid long test runs.

## Update Triggers
- New features or protocol changes.
- Changes to clipboard handling or agent behavior.
- Adding/removing dev-dependencies for tests.

## Related Docs
- `docs/troubleshooting.md`
- `docs/cli.md`
