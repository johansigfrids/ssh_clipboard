# Docs Index

## Purpose
Internal knowledge base for maintainers and AI agents working on `ssh_clipboard`. These are not end-user docs.

## Key Files
- `docs/index.md`
- `docs/writing-docs.md`
- `AGENTS.md`
- `ARCHITECTURE.md`
- `IMPLEMENTATION_PLAN.md`

## Where To Start
- Skim this index, then jump to any topic docs listed below as they are added.

## Docs Map
- `docs/writing-docs.md`: How to write and maintain internal docs in this repo.
- `ARCHITECTURE.md`: System architecture and implementation roadmap.
- `IMPLEMENTATION_PLAN.md`: Phased implementation checklist.
- `docs/protocol.md`: Protocol framing, message types, and error codes.
- `docs/server-setup.md`: Linux daemon setup and proxy usage.
- `docs/client-setup.md`: Windows/macOS client usage and SSH troubleshooting.
- `docs/security.md`: Security model and recommended SSH hardening.
- `docs/cli.md`: CLI command and flag reference.
- `docs/troubleshooting.md`: Common errors and fixes.

## Doc Conventions
- Each doc includes: `Purpose`, `Key Files`, `Update Triggers`, `Related Docs`.
- Prefer concrete paths and commands (e.g., `src/`, `cargo test`).
- Avoid secrets; reference where secrets/config are managed instead.

## Update Triggers
- Behavior changes (clipboard transfer, SSH handling, client/server workflows).
- New or changed dependencies or OS-specific steps.
- Bug fixes that clarified edge cases or error handling.

## Related Docs
- `docs/writing-docs.md`
