# SSH Clipboard Rework Implementation Plan

## Purpose

Turn the October 2026 redesign into a staged implementation that preserves the core idea: explicitly share clipboard contents between machines through SSH. Address forgotten shortcuts, file-based settings, and cumbersome updates as product problems rather than isolated fixes.

**Status: Windows/Linux rollout delivered, updated 2026-10-16.** The shared configuration, quick panel/settings, bounded shelf protocol, persistent sessions, metadata recovery, cancellation and uncertain-share reconciliation are implemented. Earlier Windows panel, configuration, image paste, profile controls and direct Share checks are user-confirmed; the 2026-10-16 isolated native shelf/migration pass now confirms text/PNG paste, passive history/privacy, legacy switching, direct Copy, cancellation/reconnect and acknowledgment lookup. Its omitted paths remain explicit. Headless process/fault tests and disposable real SSH exchanges pass. Desktop signed staging/updater and stable server maintenance/draft workflows are implemented, and the owner has configured production signing inputs and environment protections. Synthetic package proofs and a disposable real user-systemd runtime test pass, including crash-loop rate-limit rollback, journal recovery and cleanup with the installed service/SSH route preserved. Isolated Windows installation/startup, real sign-in, built-in upgrades, cancellation/tamper rejection, installed signing-key transition and post-update text share/copy now pass, with test cleanup complete. The one-time legacy-import review is implemented with automated persistence/race/rendered evidence. Actual published Linux x86_64 v0.3.0 CLI/proxy/daemon compatibility now passes against the new routes in private process tests. Protected production signing, anonymous public delivery and the permission-reviewed clean live server installation now pass. Desktop 0.4.1 and server 0.4.0 are published; the installed desktop history profile is selected. Actual server reboot/last-session logout, interrupted native installation and broader platform acceptance remain explicit gaps. Phase sections and [ARCHITECTURE.md](../ARCHITECTURE.md) record evidence separately from the target system.

## Key Files

- `src/agent/`, `src/bin/ssh_clipboard_agent.rs`: existing tray, shortcuts, settings, and autostart.
- `src/client/`, `src/client_actions.rs`: clipboard capture/write and SSH transport.
- `src/protocol.rs`, `src/framing.rs`, `src/daemon.rs`, `src/proxy.rs`: protocol and server.
- `src/cli/`: CLI, diagnostics, and existing installers.
- `Cargo.toml`, `.github/workflows/`: eventual workspace/dependency and release changes.
- [Architecture](../ARCHITECTURE.md): target contracts and component boundaries.

## Accepted Direction

1. Make the **quick panel** the primary desktop interface. A small recent list belongs inside it; a large transfer dashboard is not the default.
2. Use one discoverable shortcut to open the panel, with tray/menu and normal launcher access. Keep direct share/copy shortcuts optional and visible in the UI.
3. Provide native-feeling settings for connections, shortcuts, behavior, and updates. Desktop and CLI share configuration and diagnostics.
4. Replace the single remote slot with a bounded, temporary, in-memory shelf of explicitly shared items. Manual share/copy remains the default.
5. Keep system OpenSSH, the Linux hub, per-user UNIX sockets, and account isolation. Reuse SSH aliases/keys instead of creating another identity service.
6. Build a shared Rust core, desktop shell, headless CLI, and headless Linux server with explicit boundaries.
7. Add persistent desktop sessions, capability negotiation, useful failure states, and safe reconnection. CLI operations may stay one-shot.
8. Require free artifact signing for updates. Paid Windows/macOS publisher signing is a separate future choice.

The mockup is an interaction reference, not a literal platform specification. Its document names, device names, versions, and update progress were sample data. Content labels must come from actual content or user input.

## Decisions to Prove Before Committing

| Choice | Working proposal | Decision evidence |
| --- | --- | --- |
| Desktop framework | Tauri 2 + Svelte/TypeScript, Rust backend | Native panel, clipboard ownership, tray, shortcuts, installer, and update prototype on supported platforms. |
| Initial retention | 20 items, one hour, 256 MiB total encoded payload | Representative text/screenshots and bounded-memory tests; keep values configurable and bound transient allocations separately. |
| Initial per-item limits | Configurable 64 MiB encoded item and 256 MiB decoded pixels for the new client/server; retain explicit 10 MiB legacy compatibility | Native image testing exposed the old limit as too restrictive. Test screenshots/photos, boundary behavior, and actual transient memory use; negotiate server limits and account for metadata separately. |
| New wire contract | Separate session endpoint, negotiated protocol/capabilities | Compatibility tests including forced-command keys; pin bootstrap framing and codec fixtures. |
| Linux distribution | AppImage for built-in desktop updater; packages use package manager | Packaged smoke tests on selected X11/Wayland desktops. |
| Release trust | Embedded public key; protected release-only private key | Tampering, wrong-key, replay/downgrade, and rotation rehearsal. |

Choose the desktop framework after the prototype gate. Do not rewrite the whole agent around unverified platform assumptions. Keep optional automation, disk history, full-mesh networking, HTML/RTF, OCR, AI naming, and file transfer outside the first release.

## Phase 1: Prove the Desktop Foundation

**Windows smoke checks passed:** `desktop/` contains the Tauri/Svelte candidate with version 2 transfers, session settings, previews, a retained clipboard worker, tray/shortcut/single-instance integration, and automated tests. Windows compilation, browser UI checks, and an isolated real SSH round trip against Linux server 0.3.0 pass. The user confirmed native image paste, focus, scrolling, shortcut/tray reopening, and second-launch behavior. Broader fault/conflict checks and macOS/Linux results remain pending; see [validation status](desktop-prototype.md#validation-status). Do not treat this prototype as a completed framework decision or a replacement installer.

Build a minimal desktop spike against existing text/PNG transfer services. It should open a compact panel, display a real item, share/copy, open settings, and retain native clipboard ownership. Add one panel shortcut and launcher/tray access. Record supported OS versions, architectures, and representative Linux desktops before choosing the framework.

Validate panel placement, keyboard navigation, focus/dismissal, multi-monitor scaling, single-instance activation, and app behavior with no tray. Copy closes or leaves the panel according to an explicit preference; it must not attempt to paste into another application automatically.

On Linux, probe tray, X11 shortcut, and GlobalShortcuts portal capabilities independently. Missing/denied capabilities produce explanations and working alternate entry points. Establish how CLI writes preserve clipboard contents when a live owner is required.

**Deliverables:** small prototype, platform results, framework/clipboard decisions in `docs/dependency-decisions.md`, and updated platform support notes. The spike can be replaced; it is not the shipping architecture.

**Exit checks:** forget every keybind and still open/share/copy through visible controls; paste text and images into another native application after the panel is dismissed; shortcut conflicts and portal denial leave the app usable. Record native results separately from simulated clipboard tests.

## Phase 2: Extract Shared Services and Configuration

**Foundation slices implemented:** a shared versioned configuration store and persistent desktop connection/panel preferences, with coordinated saves, stale-edit rejection, and corrupt-file preservation. Windows persistence and live CLI reload are user-confirmed. CLI settings commands edit the same store, add/select profiles, and resolve transfer defaults with explicit overrides. Desktop live reload defers changes during operations and guards stale drafts/transfers with revisions. [Build variants](build-variants.md) now separate OS clipboard/GUI dependencies from the headless server/stdio CLI; the Linux-target dependency graph is verified, with native headless Linux compilation/process tests now passing through the existing WSL toolchain. See [shared configuration](shared-configuration.md). These slices proceed provisionally while other platforms' Phase 1 checks remain open; they do not finalize the framework choice. Schema 2 now includes backed-up legacy/schema 1 migration, advanced transport policy, device identity, and retained startup/direct-shortcut/update preferences. Core Phase 2 implementation is in place; on 2026-10-14 the user confirmed the schema 2 Apply/Quit/relaunch and small-image Share/Copy/paste checks in the standalone Windows artifact. Headless Linux checks now pass in WSL; desktop/platform acceptance remains open. Profile/advanced/startup controls belong to Phase 3, with updater activation in Phase 6. See [migration/recovery](configuration-migration.md).

Extract focused Rust services for configuration, profiles, clipboard adapters, item models, transport, and transfer actions. Make desktop and CLI use them while retaining the current protocol initially. Move GUI and OS clipboard dependencies out of the server dependency graph; keep only image-codec functionality needed for safe server-side validation.

Use a Cargo workspace when it provides real dependency isolation. Suggested eventual boundaries are `protocol`, `client-core`, `platform-clipboard`, `cli`, `desktop`, and `server`; exact crate names are provisional. Avoid a large file move with no usable behavior at the end.

Introduce a versioned shared configuration schema with stable profile IDs, default profile, SSH target/alias and options, device identity/display name, shortcuts, start-at-login, preview preferences, and update policy. Define CLI override precedence explicitly: defaults, selected profile/configuration, then command-line overrides.

Migrate the existing agent configuration into a default profile with a backup. Never overwrite a corrupt file with defaults. Coordinate writers, persist atomically, and notify/reload the desktop when CLI settings change. Report migration failures with a recovery path. Preserve connection options and the user's autostart choice.

**Deliverables:** shared services, migration, configuration CLI parity, dependency isolation, and configuration documentation.

**Exit checks:** desktop and CLI select the same target; repeated migration is safe; corrupt configuration survives; interrupted saves and concurrent edits do not lose settings; the server builds without desktop libraries. Existing version 2 wire fixtures remain unchanged.

## Phase 3: Ship the Quick Panel and Settings Against the Existing Server

**Connection-profile slice implemented:** Settings can add, select, rename, and remove named connections. Selection stays in the draft until Apply; removal needs confirmation and keeps at least one profile. New profiles have independent defaults, while edits preserve existing advanced policy. GUI saves and external reloads share the same snapshot-isolation boundary. CLI removal uses the same service. Automated persistence, failure, and rendered UI probes pass. On 2026-10-14 the user confirmed native Windows profile creation/Test, switching with preview clearing, rename/relaunch persistence, and removal back to the original connection. Advanced connection controls now edit ordered SSH options, executable, timeout, encoded limit, and framing recovery through lossless decimal-string drafts and shared validation. Shared desktop/CLI diagnostics now use only metadata through the proxy, show per-step evidence, and provide safe failure hints. Automated/fault checks and a real metadata-only SSH check pass. Optional direct shortcuts now have independent recorders, explicit opt-in, background dispatch, and grouped registration/rollback. See [shortcut contracts](desktop-shortcuts.md). Native advanced/diagnostic/direct controls are deferred to the combined daily-use pass; feedback preferences, private native notification delivery, and retained outcomes are now implemented with explicit portable Windows fallback. Actual OS startup remains tied to stable installation in Phase 6.

Use the shared services to replace the old agent UI. Start with latest-item support on a legacy server; show shelf actions only when supported. The panel exposes Share current clipboard, Copy to this computer, connection state, source/time where known, and a clear empty state.

Implement these settings pages:

| Page | Required behavior |
| --- | --- |
| Connection | Named profiles, SSH alias/target, optional port/key reference, advanced options, Test connection, actionable errors. |
| Shortcuts | Recorder for Open panel, optional direct share/copy, registration result, enable/disable, restore shortcut defaults. |
| Behavior | Start at login with actual OS status, preview visibility, notifications, panel dismissal behavior. Sharing stays explicit. |
| Updates | Running version, check policy, release notes, check/download/install states; signed installation is enabled in Phase 6. |

Apply settings from a validated draft without restart. If OS shortcut/autostart registration fails, show the failure and keep active/saved state consistent. Test connection separates SSH/authentication, proxy, daemon, and protocol health and works with restricted proxy-only keys. Host-key trust and key unlocking need a deliberate native/terminal flow; never suppress verification to make the test green.

Labels use local deterministic previews: first non-empty line (approximately 60 characters), URL host/path, command/code first line, or image dimensions and thumbnail. Preserve payload bytes exactly. Escape text in the frontend, bound thumbnail decode, and do not invent document titles or fetch external page titles. Label editing can be deferred.

**Deliverables:** usable quick panel, live settings, normal launcher/single-instance behavior, sanitized diagnostic presentation, and updated client/agent docs.

**Exit checks:** complete setup and daily share/copy without editing a file; conflict a shortcut and recover in settings; switch profiles without showing/copying the previous profile's item; opening/previewing never shares or overwrites the clipboard. Legacy servers show reduced features honestly.

## Phase 4: Implement the Shelf and New Protocol

**Native endpoint slice implemented:** experimental Linux `shelf-daemon`/`session-proxy` commands now run on a separate private socket, with bounded listener tasks/expiry, bootstrap validation, fixed-buffer bidirectional forwarding, and explicit stdio process termination. Shared ownership guards prevent both daemon modes from detaching a live socket. The existing WSL toolchain provides native Linux headless/process evidence; no remote service was changed. See [endpoint evidence](shelf-server.md). The optional V2 compatibility socket now shares the same store, memory/admission budgets, and expiry timer. Mixed legacy/new proxy process tests cover exact text/PNG bytes and safe refusal of an occupied legacy endpoint. The shared sequential client and explicit shelf CLI now provide metadata history, validated file/stdout fetch, file/stdin share, and epoch-guarded delete/clear. Stream and WSL command/process probes pass; [client contracts](shelf-client.md) distinguish this from desktop and real SSH acceptance. The panel now has an explicit per-profile shelf endpoint, bounded content labels, source/expiry rows, selection, confirmed delete/clear, and selected-item availability checks before copy. Automated and default/minimum-size browser checks pass; [desktop history evidence](desktop-history.md) now includes the closed Windows real-SSH/native pass with explicit omitted paths and separate installed cutover gates.

The transport-independent [session service](shelf-service.md) now connects the codec and shelf for repeated share/list/fetch/delete/clear/health requests. Duplex tests cover cross-client deduplication, pagination races, resource pressure, deadline cleanup, and native PNG decode budgets. Native listener/proxy process checks now pass separately in WSL. Disposable real SSH and the [Windows native pass](native-history-acceptance.md) now provide separate endpoint/desktop evidence; duplex tests alone are not native service acceptance.

**Core foundation implemented:** `src/shelf.rs` now provides stable item/epoch IDs, server ordering, monotonic expiry, count/byte eviction, immutable fetched leases, reserved upload memory, bounded PNG validation, and retry deduplication with fail-closed window saturation. Sixteen platform-independent behavioral/resource probes cover state, resources, and bounded/exact-byte labels. A separate SCBS bootstrap, JSON/raw-payload codec, decimal-string integers, negotiated limits, and initial paged message model now have fixtures and malformed-input tests; see [session contract](session-protocol.md). See [core contracts and remaining work](shelf-core.md). It now powers the separate experimental Linux endpoint, with item-aware CLI and desktop history source integration implemented; the [Windows native report](native-history-acceptance.md) records its bounded acceptance scope, and the installed server and version 2 wire contract remain unchanged.

Specify the new protocol before changing serialized enums. Reserve a separately selectable session endpoint and freeze its bootstrap handshake: supported versions, capabilities, sizes, server epoch, and retention policy. Keep the existing version 2 endpoint and fixtures intact during migration. Update `docs/protocol.md` with exact bytes, failure codes, and compatibility behavior.

Add item IDs, origin metadata, server receipt time/expiry, ordered sequence numbers, and representation descriptors. Implement share, list, fetch-by-ID, delete, clear, and health. A legacy `Get`/`PeekMeta` sees the latest item; a legacy `Set` creates an item with one representation and unknown/legacy origin. Legacy clients still have no history controls.

Enforce count/age/aggregate storage limits, metadata limits, frame limits before allocation, and decoded-image limits with checked arithmetic. Validate UTF-8 and PNG safely. Bound in-flight uploads, snapshot copies, deduplication records, connections, and queued messages as well as stored bytes. Use oldest-first eviction; reject an individually oversized item rather than disrupting existing items unnecessarily.

Apply size policy consistently in the client, SSH proxy, and daemon. The current 10 MiB limit is a legacy default, not a requirement of SSH. New-session capability negotiation advertises effective item, frame, decoded-image, and aggregate limits. Reserve bounded frame space for metadata in addition to the encoded-item budget. Show measured sizes and a useful next action when rejected; do not silently resize images or lower fidelity. A client-only increase must not be presented as working against an unchanged legacy daemon.

Make share commits atomic and deduplicate retries of an upload operation ID within a bounded window. Use server sequence numbers for ordering, expiry based on server-controlled time, and a new epoch after restart. Protect daemon/socket ownership so a second daemon cannot detach the first.

**Deliverables:** shelf daemon, compatibility adapter, item-aware CLI, list/delete/clear UI, protocol fixtures, and updated server/security docs.

**Exit checks:** simultaneous clients converge on the same order; expiry/eviction refresh the panel; exactly-at-limit content behaves according to documented limits; invalid images/lengths fail without uncontrolled allocations; duplicate retries do not duplicate items within the deduplication window; duplicate daemon startup preserves the original service. Restart clears the shelf and is visible to clients.

## Phase 5: Add Persistent Sessions and Reconnection

**Implemented slices:** the shared owner and desktop now reuse one negotiated SSH session for shelf actions, with bounded lifetime stderr, profile/configuration invalidation, failure reaping, and no automatic mutation replay. Share freezes one snapshot before waiting for SSH. The desktop now exposes lock-independent Cancel, operation identities, stale-event rejection, finishing boundaries before native writes/after shelf acknowledgments, and conservative lost-acknowledgment feedback. Native admission survives a cancelled waiter, and Open panel remains available during a transfer. Portable/native process and rendered probes cover these contracts; see [owned sessions and cancellation](persistent-sessions.md). A bounded dedicated metadata connection now snapshots/watches history and reconnects transient faults with capped backoff; trust/authentication/protocol failures pause for explicit action. Profile/revision/generation and user-action barriers reject obsolete replies. Lost acknowledgments now have metadata-only operation lookup plus explicit frozen-capture retry/discard. Both the original upload and retries use one server-enforced monotonic deadline, with bounded RAM retention and epoch checks; see [share recovery](share-recovery.md). The [2026-10-16 Windows session](native-history-acceptance.md) confirms history, background Open/Cancel, reconnect and read-only lost-ack recovery. Explicit captured retry/discard and server-restart epoch UI remain automated-only evidence.

The legacy one-shot transport already has bounded concurrent stderr drainage, a single operation deadline, reply ID/kind validation, direct-child cleanup, typed failures, and conservative uncertain-share messages. These guardrails were pulled forward to support Phase 3 diagnostics. They do not implement persistent sessions, subscriptions, reconnection, deduplicated retries, or a Cancel control. See [transport evidence](connection-diagnostics.md).

Extend the new endpoint to handle multiple requests and metadata subscriptions. Keep one active profile with one foreground data connection and one bounded metadata control connection; fetch preview/payload bytes only when needed. The separate control owner prevents a long-poll Watch from blocking user transfers in the sequential protocol. Retain one-shot CLI use and explicit legacy mode.

Drain bounded SSH stderr concurrently with protocol reads. Verify request/reply correlation, maintain an operation deadline, expose cancellation, and reap SSH processes. Use bounded operation/subscriber queues, backoff, and idle health policy. Do not hold the shelf lock across socket writes or large image work.

Specify how an initial metadata snapshot and subscription avoid missed events; handle sequence gaps and server epoch changes by resnapshotting. Slow consumers receive a resync/disconnect outcome instead of unlimited buffering. An upload acknowledgement lost during a disconnect is reconciled by operation ID while supported; otherwise report an uncertain outcome.

Reconnect refreshes remote state and never silently samples/uploads the local clipboard. Do not replay clipboard writes after reconnect. Obsolete replies/fetches must not affect a new profile or overwrite a newer user action.

**Deliverables:** explicit connection state machine, subscription UI refresh, cancellation, transport fault tests, and diagnostic mappings.

**Exit checks:** test SSH rejection, noisy shell, large stderr, missing daemon, stalled stream, dropped reply, wrong response ID, sequence gap, daemon restart, slow reader, and profile change during a transfer. Each produces bounded cleanup and an understandable result. Run a real SSH integration test as well as process/stream simulations.

## Phase 6: Build Stable Desktop Packaging and Signed Updates

**Implemented source slices:** strict package verification authenticates exact bytes and signed version/platform/channel. Bounded HTTPS discovery/download feeds an explicit Updates page and a one-use local handoff to the framework installer; new transfers and unresolved shares guard installation. Staging bundles a matching CLI, signs canonical assets, and verifies keyed feeds/checksums; isolated Windows NSIS debug builds pass. Startup controls read actual registration and coordinate explicit changes/save rollback. A manual master-only environment-gated workflow creates signed drafts without publishing. The owner has configured production signing inputs and environment protections; isolated native Windows installation, sign-in, built-in upgrades, download cancellation, tamper rejection and installed key transition pass. Protected 0.4.0 signing, independent package verification and production Windows install/text paste now pass. Windows desktop 0.4.1 publication/public delivery pass; native installation interruption and other-platform packages remain open; see [update trust evidence](desktop-updates.md).

Package the chosen desktop framework at a stable per-user location, with configuration outside the replaceable application files. Bundle or install the CLI through a documented path; prevent drift between bundled CLI/core versions. Start-at-login points to the stable installed entry point, and replacing the app preserves the choice.

Use the framework's supported Windows/macOS updater packages and Linux AppImage updater. Package-manager installations use their package manager. Define minimum supported platform/runtime versions from the prototype results.

Set up free update signing when implementation of release infrastructure is authorized:

1. Generate a dedicated signing keypair, independent of SSH keys. Embed the public key in the app; keep a secure private-key backup outside source and build output.
2. Put the private key in protected release-only credentials. Normal builds/PR jobs never receive it. Restrict signing to approved release refs and trusted workflow/dependency inputs.
3. Build and test target-specific packages; sign the exact bytes that will be downloaded. Publish artifacts, signatures, release metadata, and checksums through GitHub Releases as one complete release.
4. Verify a downloaded artifact before installer execution or file replacement. Fail closed for missing/invalid signatures, unexpected platform/channel, and disallowed versions.
5. Define which release metadata is authenticated. If framework signatures cover only package bytes, add authenticated metadata for security-relevant version/channel/platform decisions rather than claiming the whole feed is signed.
6. Rehearse key rotation through an old-key-trusted transition release. Document recovery for lost/compromised keys and trusted first installation. A backup is not a rotation strategy.

The Updates page checks quietly, shows notes/version/status, and installs only after active transfers finish. Disable new transfers during replacement and restore normal availability after failure where possible. Revalidate preserved configuration on startup; do not erase it to recover. Test interrupted downloads/installation and retain a documented verified recovery path. Do not promise automatic desktop rollback until platform installers actually support and demonstrate it.

**Deliverables:** installers, signed updater, protected release workflow, update policy, key lifecycle runbook, and rewritten release instructions. Replace the current delete-release/tag rollback guidance with verified recovery procedures and new corrective releases.

**Exit checks:** clean install and an installed old-to-new upgrade on each shipped platform; stable launcher/autostart/configuration; interrupted download and install; altered artifact, wrong key, missing signature, wrong platform, forged version metadata, and replay/downgrade refusal. Exercise a trusted key transition. Unsigned OS publisher status is described accurately and does not bypass updater verification.

## Phase 7: Make Server Installation and Maintenance Reliable

**Implemented source slices:** private immutable releases, atomic selection, writer ownership, durable recovery, downgrade refusal, and restart/health rollback. Concrete signatures bind server bytes to version/platform/channel/ELF architecture. Explicit Linux install/update/status/recover commands pin trust/UID/socket/limits, bound download/process lifecycles, and create a stable launcher/actual unit. Signed server staging and an environment-gated Linux x64 draft workflow are implemented. Disposable signed executable tests with fake systemctl, real proxy health and systemd unit parsing, plus synthetic package staging, pass. Production signing inputs/environment protections are configured; real user-systemd runtime acceptance now passes with crash-loop rollback and complete cleanup. Protected 0.4.0 signing and exact optimized-package admission/isolated runtime service acceptance now pass. The owner-authorized signed live 0.4.0 installation and clean legacy retirement now pass; public server delivery also passes; actual reboot/last-session logout remain open. See [live acceptance](linux-cutover-acceptance.md). See [maintenance evidence](server-maintenance.md).

Replace symlinks into extracted archives with user-owned versioned releases, a stable launcher, and a stable user service file. Migrate the existing user service carefully, preserving target UID and socket/configuration. Do not rewrite shared global installations automatically.

Add a maintenance command that downloads/stages a platform-correct signed package, verifies it, switches the active version, restarts the user service, checks protocol health through the proxy, and switches back if health fails. Keep the previous executable and record the update outcome. Failed updates must not strand the stable launcher.

The desktop may show server version and maintenance instructions. It can perform remote maintenance only with a separately authorized management connection. A forced-command clipboard key remains restricted; health checks use the transfer protocol. Restart clears the in-memory shelf even if the executable is later rolled back, so show this before maintenance.

**Deliverables:** stable installer/maintenance command, server artifact signing/verification, executable rollback, service migration, and server setup runbook.

**Exit checks:** installation survives removal of the download folder; correct socket/user permissions remain; signature failure leaves the running service untouched; restart/health failure restores the executable/service; disk interruption preserves a bootable active version. Validate on a disposable Linux installation before any live-host rollout.

## Phase 8: Validate Migration and Cut Over

**Implemented migration slice:** a one-time legacy-import review now appears in the panel and Settings with the current connection/device, panel shortcut, retained direct bindings/startup choice and recovery-backup path. Explicit dismissal saves a guarded review receipt without changing OS registration or active transfer state. Automated persistence, failure, stale-write and rendered checks pass. The [2026-10-16 Windows pass](native-history-acceptance.md) also confirms the synthetic import review, dismissal and relaunch persistence. Import of the actual user legacy file and installed cutover remain separate.

**Released compatibility slice:** [pinned v0.3.0 Linux process probes](released-compatibility.md) now prove both protocol upgrade orders using the actual published CLI/proxy/daemon. Exact text/PNG, latest/history mutations, fixed legacy-route refusal and response-budget behavior pass with owned cleanup. The Linux server CI job repeats these checks against its optimized binary. Installed migration, real SSH-key changes and other released platforms remain separate gates.

**Prepared:** the [compatibility/cutover checklist](compatibility-and-cutover.md) maps actual endpoint/configuration behavior, server-first/desktop-first sequences, SSH privilege boundaries and return paths. Installed/released-client/native/service acceptance and the separately authorized live cutover remain open. No support-window end date or legacy-agent removal has been chosen.

Publish a compatibility matrix covering new/legacy clients and servers, supported protocol ranges, platform packages, and forced-command key setup. Release version numbers and the support window are decided from the tested migration path, not from mockup sample versions.

Test server-first and desktop-first upgrades, existing configuration, custom SSH/socket options, autostart enabled/disabled, and protocol-only keys. New clients can select legacy mode with reduced features; unsupported new-only actions must explain why they are unavailable. A fixed legacy forced command needs an explicit reviewed change to expose the new endpoint.

Offer a one-time migration summary for preserved connection settings, device name, the new panel shortcut, and optional legacy direct shortcuts. Replace the old agent's autostart/single-instance behavior only after the new app works; avoid starting two agents or having both register the same shortcuts. Retain a documented return path to the old client/configuration during the migration window.

**Deliverables:** final installation/usage docs, compatibility matrix, release notes, platform evidence, and cleanup of the obsolete agent after the support window. Update `README.md`, feature docs, `ARCHITECTURE.md`, and the docs index as work lands.

**Exit checks:** a user can install, connect, share, copy, change settings, and update without remembering a keybind or editing configuration; another machine sees shelf changes; offline/reconnect behavior is understandable; all supported upgrade combinations pass. Publication and live rollout remain separate explicitly authorized actions.

## Release Preparation Status

On 2026-10-16, the 0.4.0 rework merged at `766f6e2` after Windows core/desktop, frontend, headless Linux and pinned released-client validation. Both approved protected workflows created independently verified signed drafts. Production Windows installation/text paste and isolated exact-artifact Linux SSH/service checks pass. The owner authorized legacy Windows agent removal, native startup cleanup and the desktop 0.4.1 correction; actual login with the quoted command passed. See [0.4.0 acceptance](release-0.4.0-acceptance.md).

Desktop **0.4.1** merged at `80051ad`, passed exact-source CI/protected signing/independent package verification, and was installed through the ordinary Windows desktop. The owner confirmed version, retained connection and startup off/on. After separate publication approval it was published as latest, with anonymous public installer/signature/checksum/feed verification. See [0.4.1 acceptance](release-0.4.1-acceptance.md). The immutable 0.4.0 desktop draft remains held; the independent server-v0.4.0 is published with anonymous public-delivery verification and the desktop latest/feed preserved. Documentation-only follow-ups do not change signed artifact source.

The legacy client and V2 route remain supported throughout the 0.4 series. The signed Linux 0.4.0 managed service and user SSH route now pass live acceptance, with the old active installation removed. Public server delivery also passes; actual reboot/last-session logout remain open; see [live cutover acceptance](linux-cutover-acceptance.md). Interrupted native installation, omitted native recovery paths and other-platform desktop packaging retain their stated gaps. The manual Windows history session is closed; no repeat paste/history/sign-out session is required for the startup-only correction. See [release readiness](release-readiness.md).

The owner-requested clean Linux 0.4 installation is complete: managed shelf and fresh SSH routes pass; the old daemon/unit links, extracted binaries/archive and obsolete global link are retired, with one separate private recovery backup. The new V2 compatibility adapter remains. The owner confirmed selecting and saving the installed Windows history endpoint. See [cleanup boundaries](compatibility-and-cutover.md#owner-requested-clean-server-replacement).

## Cross-Cutting Verification

The next owner-requested platform cutover is a complete Mac 0.3-to-0.4 replacement: preserve settings, use the current history server, establish installed login startup, and retire the old client/startup/configuration after native acceptance. The 0.4.2 preparation merged at `b465c44` after all seven candidate CI checks; the actual Mac passes signature/CLI/copied-import and metadata health with its owner-unlocked GUI agent. The protected production run now passes all five jobs and independent verification of both signed packages and the combined draft. The exact Mac app and migrated history settings are installed with a verified private backup. The [Mac cutover plan](macos-cutover.md) records the stale LaunchAgent, missing old executable and remaining GUI/login/update/cleanup gates. A permanently parallel test installation is not the intended outcome.

- Keep current Rust formatting, clippy-with-warnings-denied, protocol fixtures, and agent/server test variants green during transition. Update commands and CI as workspace targets change; add frontend type/build checks when the frontend is introduced.
- Test observable contracts: preserved clipboard bytes, native paste availability, configuration recovery, bounded resource use, no accidental upload/copy, profile isolation, and cryptographic rejection before installation.
- Distinguish unit/process tests, real SSH tests, native desktop tests, and installed-artifact upgrade tests. Record what was actually run on each platform and any unverified gap.
- Every behavior change updates its existing feature documentation and captures root cause/guardrails for bugs found along the way. Never turn planned commands or defaults into current usage instructions before they exist.

## Initial Implementation Slice

Start with Phase 1, then Phase 2 and the panel/settings in Phase 3 against the existing server. This proves the daily interaction and platform foundation before protocol migration. Phases 4-5 add the shelf/session model; Phase 6 completes desktop signed updates; Phase 7 handles server maintenance; Phase 8 gates the final replacement.

Keep each slice reviewable and usable. The rework is complete only when discoverable interaction, real settings, signed installed updates, and safe cross-version migration all work; a new frontend alone does not meet that bar.

## Update Triggers

- A phase is implemented, reordered, or fails its prototype/acceptance gate.
- Platform capability, packaging, retention, codec, or key lifecycle decisions are finalized.
- New migration constraints or native/installed tests change the required scope.

## Related Docs

- [Architecture](../ARCHITECTURE.md)
- [Docs index](index.md) and [writing conventions](writing-docs.md)
- [Dependency decisions](dependency-decisions.md)
- [Protocol](protocol.md) and [security](security.md)
- [Agent](agent.md), [autostart](autostart.md), and [Linux desktop](linux-client.md)
- [CLI](cli.md) and [server setup](server-setup.md)
- [Testing](testing.md), [CI](ci.md), and [releasing](releasing.md)

## Design References

Consult current official documentation again when choosing dependencies and implementing the relevant phase:

- [Tauri updater and update signing](https://v2.tauri.app/plugin/updater/)
- [Tauri tray integration](https://v2.tauri.app/learn/system-tray/)
- [Tauri capability permissions](https://v2.tauri.app/security/capabilities/)
- [Global hotkey platform support](https://github.com/tauri-apps/global-hotkey)
- [XDG GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)
- [Windows publisher signing](https://v2.tauri.app/distribute/sign/windows/)
- [macOS signing/notarization](https://v2.tauri.app/distribute/sign/macos/)
