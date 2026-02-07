# Dependency Decisions

## Purpose
Capture high-impact dependency choices, their rationale, and guardrails for future upgrades.

## Key Files
- `Cargo.toml`
- `src/framing.rs`
- `src/protocol.rs`
- `docs/protocol.md`

## bincode -> wincode (2026-02)

### Decision
- Replace `bincode` with `wincode` for protocol payload serialization.

### Why
- `bincode` `3.0.0` is intentionally non-functional and emits a compile error.
- The crate is marked unmaintained, so staying on `2.x` long-term is a maintenance risk.
- `wincode` is bincode-compatible for the used protocol shapes and is actively maintained.

### Compatibility Requirement
- Protocol wire version remains `2`.
- Serialized bytes for representative `Request`/`Response` fixtures are locked by tests in `src/protocol.rs`.
- Any codec/config change must preserve fixtures or explicitly bump protocol version and update docs.

### Implementation Guardrails
- Codec config is centralized in `src/framing.rs` (`codec_config()` + `CodecConfig`).
- Config intentionally uses little-endian + varint to match prior behavior.
- Preallocation limit is intentionally disabled to avoid introducing a new 4 MiB serialization/deserialization cap.

### Upgrade Checklist (Codec-Related)
- Update dependency in `Cargo.toml`.
- Verify `cargo test --features agent` and `cargo test --no-default-features`.
- Verify fixture tests in `src/protocol.rs` still pass.
- If wire bytes change:
  - decide whether this is acceptable,
  - bump protocol version if needed,
  - update `docs/protocol.md` and release notes.

## Update Triggers
- Serialization dependency upgrades or replacements.
- Protocol encoding/config changes.
- New compatibility incidents discovered in CI or release testing.

## Related Docs
- `docs/protocol.md`
- `docs/testing.md`
- `docs/releasing.md`
- `docs/index.md`
