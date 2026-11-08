# Dependency Decisions

## Purpose
Capture high-impact dependency choices, their rationale, and guardrails for future upgrades.

## Key Files
- `Cargo.toml`
- `src/session_protocol/framing.rs`
- `src/session_protocol.rs`
- [Session protocol](session-protocol.md)

## Session Serialization

SCBS uses strict bounded JSON metadata and raw payload frames. Historical [V2 serializer decisions](history/0.4/dependency-decisions-v2.md) do not describe current dependencies.

## Desktop Foundation (2026-10)

### Windows Startup Command Boundary (2026-10-16)

The installed desktop uses a direct, Windows-only optional `auto-launch` dependency under `installed-desktop`, at the same 0.6.0 version already pinned transitively by Tauri. No package version changed in the lockfile. The pinned Tauri wrapper passes an unquoted executable into that backend's literal command concatenation. The Windows adapter now supplies a quoted absolute path and explicit current-user enable mode; other platforms keep the Tauri manager. Keep the native Windows argument-parser regression, saved-settings rollback tests and actual login evidence separate. A dependency upgrade must recheck the generated command rather than assuming that registry presence proves launchability. See [startup evidence](desktop-updates.md#start-at-login).

### Accepted Decision for Shipped Platforms

Tauri 2 with Svelte/TypeScript is the shipped Windows/Apple Silicon desktop. Actual owner migration, settings, native text, login/shortcut and installed update acceptance are recorded in [publication acceptance](history/release-0.4.2-acceptance.md). Separate lockfiles and an explicit desktop identifier remain. The old agent is [retired](history/0.4/legacy-retirement.md); Linux desktop and additional rare manual checks are deferred, not an unresolved framework-selection gate.

### Guardrails

- Preserve the native clipboard ownership, focus/tray/shortcut and package contracts established during acceptance. For a relevant change, use automated evidence first and a targeted native check only where needed.
- Keep clipboard ownership on one worker thread for the process lifetime; initialize access only on an explicit transfer action. Validate native paste separately from adapter tests.
- Select a TypeScript version supported by `svelte-check`; the latest TypeScript major was outside its peer range, so the desktop uses TypeScript 6. Commit the npm lockfile and use `npm ci` for reproducibility.
- Avoid unrelated dependency churn between root and desktop lockfiles. Recheck both graphs when updating shared APIs or codec/parser dependencies.
- Check session wire fixtures and shared APIs under both dependency lockfiles.
- A host consuming the library on Windows opts into `external-resource-owner`. Tauri embeds the executable's resources; embedding them again in the library causes duplicate VERSION records at link time. Normal CLI builds keep their original resources.
- Wayland currently uses visible alternate entry points, with portal support pending. Do not claim OS shortcut or tray capability from a browser screenshot or a successful compile.

See [desktop development](desktop-development.md) for build guardrails and links to dated evidence; the [implementation plan](history/implementation-plan.md) defines the closed scope and deferred work.

## Shared Configuration Storage

The shared configuration store uses `dirs` for a common per-user path and `tempfile` for same-directory atomic replacement, with standard-library file locks to coordinate writers. Keep the lock file stable across replacement and read settings without implicitly creating defaults. Only schema 3 settings.json is read; there is no source discovery or migration parser. A UUID is assigned once on durable save for future device identity. See [shared configuration](shared-configuration.md) and [fresh cutover](compatibility-and-cutover.md).

## Dependency Upgrade Guardrails

Refresh the root Cargo, desktop Cargo and npm lockfiles independently, then validate the shared APIs under both Rust graphs. Preserve PNG-only codecs, the headless core, Windows startup quoting and the explicit Rustls ring provider. Keep Tauri's Rust and JavaScript packages on the same minor version; plugin versions need not match the core's version. Keep TypeScript within `svelte-check`'s declared peer range rather than forcing TypeScript 7.

The November 2026 refresh updates Tokio and rand past their reported soundness issues, and the desktop's Rustls certificate verification, bundled roots and updater ZIP extraction past their security advisories. The npm lock also updates jsdom's transitive undici dependency past its high-severity advisories. Run both RustSec audits and the full npm audit after refreshes: a successful build alone does not check advisories, and excluding development dependencies would miss the test-tooling findings. See [audit commands](testing.md#dependency-audits).

Remaining upstream warnings must be reported separately from patched vulnerabilities. The current desktop graph retains `proc-macro-error` (unmaintained), `glib` 0.18 (`VariantStrIter` soundness), and the standalone `zbus_names`, `zvariant` and `zvariant_derive` crates (unmaintained) through GTK/D-Bus dependencies. Do not force incompatible GTK/D-Bus majors into Tauri's graph. The Vite Svelte plugin also retains `deepmerge` 4.3.1; [GHSA-gcqx-w48m-w2h4](https://github.com/advisories/GHSA-gcqx-w48m-w2h4) has no published patched version as of 2026-11-08. Retain these findings visibly until compatible upstream fixes are available, without blanket audit suppressions.

The base64 0.23 migration keeps the existing standard-alphabet engine and canonical padding checks. The dirs 7 migration preserves the existing configuration/data/home directory calls and requires no settings migration. jsdom 31 changes CSS and parsing behavior and queues DOM events as tasks; timer-cleanup tests assert that the application's own interval handle is cleared, rather than counting environment tasks or clearing all timers and hiding a leak. Native clipboard and installer acceptance remain separate from rendered tests.

## Headless Image and Clipboard Boundary

Shared PNG/image actions use a library-owned RGBA type. Only the desktop owns native clipboard payloads, adapter traits and retained ownership; the core remains headless. Disable default image codecs and select PNG explicitly, since no other image representation is supported by the protocol. See [build variants](build-variants.md) for the dependency guard and checks.

## Desktop Notification Adapter

Use Tauri's notification plugin behind Rust-only application commands, pinned in the separate desktop lockfile. Keep notification bodies static and private. Windows portable/debug builds must retain panel fallback until stable installed identity exists; the plugin's desktop permission stub does not verify OS settings or delivery. See [feedback contracts and native gaps](desktop-notifications.md).

## Shelf Retry Fingerprints

Use SHA-256 (`sha2` 0.11, locked) for bounded, content/metadata-sensitive retry records without retaining duplicate payloads. Domain-separate and length-delimit the captured upload fields. Dependency upgrades must preserve the exact digest bytes and lowercase hexadecimal release hashes. sha2 0.11's digest output no longer implements `LowerHex`; the Linux release store and public-key ID share an explicit two-digit-per-byte formatter, protected by fixed hash vectors. Retry-fingerprint vectors also freeze the existing preimage and raw digest bytes. These fingerprints detect operation-ID reuse; they are not update signatures or a replacement for SSH authentication. See [shelf retry guarantees](shelf-core.md#retry-guarantees).

## Signed Release Verification

Signed desktop/server packages use the locked `minisign-verify` implementation for exact bytes and global trusted-comment verification, with explicit version/filename bindings. Server admission additionally checks the signed artifact's Linux architecture. This verifier has no native GUI or network dependencies, preserving the headless build. Synthetic test signatures come from the existing Tauri CLI and never retain private keys. Dependency additions must preserve existing lock pins; avoid a broad `generate-lockfile`/unrelated update while adding a verifier. See [update trust](desktop-updates.md) and [server maintenance](server-maintenance.md).

## Update Triggers

- Desktop framework/dependency policy or native platform contracts change.
- Serialization dependency upgrades or replacements.
- Protocol encoding/config changes.
- New compatibility incidents discovered in CI or release testing.

## Related Docs

- [Desktop development](desktop-development.md)
- [Session protocol](session-protocol.md)
- [docs/testing.md](testing.md)
- [docs/releasing.md](releasing.md)
- [docs/index.md](index.md)
