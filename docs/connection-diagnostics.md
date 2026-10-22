# Connection Diagnostics

## Purpose

Diagnose the saved session endpoint with metadata-only Health and safe local messages.

## Key Files

- `src/client/diagnostics.rs`: session Health evidence.
- `src/client/failure.rs`: safe SSH failure classification.
- `desktop/src-tauri/src/panel.rs`: saved-connection check.

## Behavior

Desktop Test saved connection negotiates SCBS and requests Health using the saved SSH configuration and session policy. Apply a dirty draft before testing. CLI `shelf health` offers the same endpoint check. A restricted key may permit only `session-proxy`; diagnostics never add shell or maintenance probes.

Evidence distinguishes SSH launch/connection, proxy, daemon, protocol and metadata request. A failed stage must not imply unobserved later stages passed. Missing daemon and unsupported version receive explicit safe guidance. Remote stderr/server text and clipboard contents are never included in reports. Bounded stderr capture retains classification evidence while draining floods.

Health does not fetch data, sample/write the OS clipboard, enable Copy or prove a different SSH route. Authentication and host verification failures require action; no legacy fallback or automatic weakening of SSH restrictions occurs.

## Update Triggers

- Diagnostic stages, session errors, transport cleanup or SSH routing changes.

## Related Docs

- [Troubleshooting](troubleshooting.md), [sessions](persistent-sessions.md), [security](security.md).
