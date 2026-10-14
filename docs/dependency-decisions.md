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

## Desktop Foundation Candidate (2026-10)

### Provisional Decision

Evaluate Tauri 2 with Svelte/TypeScript in an isolated `desktop/` project before replacing the old agent. This provides a real settings/panel surface with the existing Rust transfer code, while keeping the framework choice reversible. The new project has separate lockfiles and a distinct application identifier. Phase 2 can migrate configuration with exact backups; installed executables and OS startup registrations remain untouched.

### Guardrails

- Native clipboard ownership, focus/tray/shortcut behavior, and platform packaging must pass the Phase 1 gate before accepting the framework for the shipping app.
- Keep clipboard ownership on one worker thread for the process lifetime; initialize access only on an explicit transfer action. Validate native paste separately from adapter tests.
- Select a TypeScript version supported by `svelte-check`; the latest TypeScript major was outside its peer range, so the prototype uses TypeScript 6. Commit the npm lockfile and use `npm ci` for reproducibility.
- Keep the existing client's `time` version in the prototype lockfile while evaluating it, avoiding an unrelated CLI deprecation from a newer compatible release.
- Check legacy codec fixtures with both dependency graphs; a separate Cargo lockfile can choose a newer compatible `wincode` version.
- A host consuming the library on Windows opts into `external-resource-owner`. Tauri embeds the executable's resources; embedding them again in the library causes duplicate VERSION records at link time. Normal CLI builds keep their original resources.
- Wayland currently uses visible alternate entry points, with portal support pending. Do not claim OS shortcut or tray capability from a browser screenshot or a successful compile.

See [the prototype guide](desktop-prototype.md) for current evidence and open checks.

## Shared Configuration Storage

The shared configuration store uses `dirs` for a common per-user path and `tempfile` for same-directory atomic replacement, with standard-library file locks to coordinate writers. Keep the lock file stable across replacement and read settings without implicitly creating defaults. `confy` supplies the exact legacy path; migration reads bounded bytes and parses TOML directly rather than using its default-creating loader. A UUID is assigned once on durable save for future device identity. See [shared configuration](shared-configuration.md) and [migration/recovery](configuration-migration.md).

## Headless Image and Clipboard Boundary

Shared PNG/image actions use a library-owned RGBA type. OS clipboard access is optional through `system-clipboard`; the old `agent` feature includes it, while the new desktop owns a retained adapter. Disable default image codecs and select PNG explicitly, since no other image representation is supported by the protocol. See [build variants](build-variants.md) for the dependency guard and checks.

## Desktop Notification Adapter

Use Tauri's notification plugin behind Rust-only application commands, pinned in the separate desktop lockfile. Keep notification bodies static and private. Windows portable/debug builds must retain panel fallback until stable installed identity exists; the plugin's desktop permission stub does not verify OS settings or delivery. See [feedback contracts and native gaps](desktop-notifications.md).

## Shelf Retry Fingerprints

Use SHA-256 (`sha2` 0.10, locked) for bounded, content/metadata-sensitive retry records without retaining duplicate payloads. Domain-separate and length-delimit the captured upload fields. These fingerprints detect operation-ID reuse; they are not update signatures or a replacement for SSH authentication. See [shelf retry guarantees](shelf-core.md#retry-guarantees).

## Update Triggers

- Desktop prototype/framework selection and native platform results.
- Serialization dependency upgrades or replacements.
- Protocol encoding/config changes.
- New compatibility incidents discovered in CI or release testing.

## Related Docs

- [Desktop prototype](desktop-prototype.md)
- `docs/protocol.md`
- `docs/testing.md`
- `docs/releasing.md`
- `docs/index.md`
