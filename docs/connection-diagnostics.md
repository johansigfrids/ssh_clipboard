# Connection Diagnostics and One-Shot Transport

## Purpose

Explain the shared metadata-only health check, safe failure reporting, and the legacy transport guardrails used by desktop and CLI. Persistent sessions/reconnection remain separate Phase 5 work.

## Key Files

- `src/client/transport.rs`: one operation deadline, stderr drain, framing/correlation, and child cleanup.
- `src/client/transport/tests.rs`: real pipe-process fault fixtures, without network or OS clipboard access.
- `src/client/failure.rs`: typed failures and bounded-output classification.
- `src/client/diagnostics.rs`: metadata-only check and per-step evidence.
- `src/cli/doctor.rs`, `tests/connection_doctor.rs`: CLI report and executable failure probe.
- `desktop/src-tauri/src/panel.rs`, `desktop/src/App.svelte`: saved connection report, operation hints, and per-step presentation.

## Shared Health Check

`diagnose` performs one version 2 `PeekMeta` exchange through the normal `ssh_clipboard proxy` command. It never requests a payload, writes a clipboard, invokes a general shell health command, or performs maintenance. An empty server is healthy. The check prepends `BatchMode=yes` without mutating the caller's saved options or changing host-trust policy.

The report separates the SSH executable, SSH connection, proxy, daemon, framing/protocol, and metadata request. Each is verified, failed, or unverified. A correlated error from the proxy can prove SSH/proxy/protocol while reporting an unavailable daemon; it must not turn the server green. An early failure leaves later layers unverified. A valid reply of the wrong kind is not a successful health check.

`doctor` uses this service, including selected-profile defaults and explicit override rules. It no longer probes `true`, `proxy --help`, or `ssh -V`. Those extra commands could produce false failures with proxy-only forced-command keys and were unnecessary for an actual clipboard round trip. A failed daemon/metadata check now returns doctor exit status 2; previously a missing daemon could be only a warning and return success. Local Linux hotkey capability notes remain separate.

## Desktop Report

Test saved connection uses the same health service and revision/operation guard as transfers. It stores a report but never creates a clipboard snapshot or enables Copy by itself. Failure updates the last connection state, returns a safe next-step message, and makes the per-step report available in Connection settings. Transfer errors use typed hints; server error messages are selected from static error-code mappings.

The report describes the last check, not a persistent/live connection. Hide it while a settings draft is dirty: a verified report for the saved target must not appear to certify another target being edited. Profile/connection/policy activation clears it with the cached item; a name-only or preference-only change can retain it. Opening settings and reloading configuration do not initiate a health check.

## Safe Failures

Transport failures distinguish missing/unstartable SSH, target/configuration, authentication, host identity, name resolution, connection refusal/network, missing proxy, deadline, framing/version, reply size/encoding/type, and request-ID mismatch. Known SSH messages provide best-effort hints. Wording/localization varies; unrecognized output produces a generic connection error rather than claiming authentication or service health was verified.

The operation continuously drains stderr but retains only its first and last 8 KiB. Raw stderr, terminal controls, decoded values, and server-supplied error text do not appear in the shared diagnostic report. Desktop server errors use static messages selected by protocol error code. Normal legacy CLI/agent protocol-error handlers retain their existing behavior; this is not a claim that every old logging path has been redesigned.

Do not disable host verification or invent an authentication prompt to make a test pass. The next step is a deliberate terminal/SSH setup action when keys or trust need attention. Never display a raw unknown protocol variant with debug formatting: it could contain clipboard bytes.

## One-Shot Lifecycle Guardrails

- Prepare/size-check the encoded request before launching SSH, including the wire length's `u32` bound. Existing version 2 fixture bytes are unchanged.
- Use one deadline spanning stream I/O, process exit, and stderr EOF. The old code restarted a full timeout for send, receive, and wait.
- Drain stderr concurrently with send/receive and process wait. Reading it only after a failed process exit could deadlock on a full pipe, including on a successful response.
- Flush and drop stdin after the one request. Shutdown alone did not reliably release the OS pipe, so an EOF-dependent proxy could wait forever.
- Reject unsupported frame versions, oversized lengths before frame allocation, malformed payloads, unmatched IDs, and response kinds inconsistent with the request. This protects CLI/agent callers as well as the desktop.
- A correlated framed `Response::Error` remains authoritative when the proxy exits nonzero. A successful reply without successful process completion is not reported as a successful operation.
- After stream/protocol failure, kill and wait for the direct SSH child. Deadline cleanup has its own two-second bound. The child retains `kill_on_drop`; stderr drainage is part of the operation future, with no detached reader task. Caller cancellation uses drop cleanup; a user-facing Cancel control and process-group management are not implemented here.
- An unacknowledged attempted share is conservatively reported as possibly committed. Refresh before sharing it again; there is no automatic replay or exactly-once guarantee. New-protocol operation-ID deduplication belongs to later phases.

## Evidence

On 2026-10-14, Windows pipe-process probes pass for a 1 MiB stderr flood, EOF-dependent completion, nonzero proxy exits with valid framed errors, early authentication failure, noisy/strict/over-budget stdout, invalid versions/lengths/encodings/types/IDs, stalled processes, and child reaping. Capture-bound and sentinel tests verify retained-output limits and safe error strings. Metadata-only service probes verify empty health, missing-daemon evidence, batch-mode precedence, and rejection of unexpected values/IDs. An executable doctor probe verifies failure status/hints without creating settings or exposing raw paths.

A real `doctor --target <authorized-server> --timeout-ms 12000` passed all seven checks against the existing Linux 0.3.0 service, using ordinary SSH credentials. It fetched no clipboard payload and changed no settings or services. A live forced-command key and native macOS/Linux builds were not exercised here. Unix pipe fixtures are included for CI; a simulated sender proves the shared health path issues only `PeekMeta`, not a live restricted-key deployment.

Rendered desktop probes verify missing-daemon versus verified SSH/proxy rows, unverified status, safe recovery presentation, report hiding/clearing during a connection edit, and successful health without enabling Copy or fetching an item. The native diagnostic/advanced-settings UI check is deferred to the combined daily-use milestone.

## Update Triggers

- Transport lifecycle, frame validation, failure kinds, or diagnostic evidence changes.
- A diagnostic starts making additional requests or gains installation/maintenance access.
- Persistent sessions, cancellation, deduplication, or live platform evidence lands.

## Related Docs

- [Protocol](protocol.md), [shared settings](shared-configuration.md), [security](security.md)
- [CLI](cli.md), [testing](testing.md), [implementation plan](implementation-plan.md)
- [Architecture](../ARCHITECTURE.md), [desktop prototype](desktop-prototype.md)
