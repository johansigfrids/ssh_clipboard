# Writing Docs

## Purpose

Keep current behavior, reusable procedures, compatibility notes and dated evidence easy to find and maintain.

## Key Files

- [Docs index](index.md): navigation for every maintained document.
- [Architecture](../ARCHITECTURE.md): component boundaries and system contracts.
- [AGENTS.md](../AGENTS.md): concise repository working instructions.

## Document Ownership and Lifecycle

| Location | Owns | Maintenance rule |
| --- | --- | --- |
| [README.md](../README.md) | Product entry point, downloads, short setup and usage | Link to detailed guides; avoid test counts and installation chronology. |
| [ARCHITECTURE.md](../ARCHITECTURE.md) | Current components and cross-component contracts | Link to feature references for details and history for evidence. |
| `docs/*.md` | Current feature contracts, development and operations | Describe implemented behavior in present tense; keep root cause and guardrail near the owning contract. |
| `docs/legacy/` | Retained agent and standalone V2 compatibility workflows | State the feature/build boundary and link to the recommended current path. |
| `docs/history/` | Completed plans, acceptance and development checkpoints | Add a historical/scope notice; preserve exact source, package, date and test boundaries. |

Current ownership is intentionally focused: shared configuration owns saves/reload; migration owns imports/backups; history owns selection/copy; sessions/recovery own transport and uncertain uploads; desktop updates owns startup/package handoff; releasing owns staging/publication/key operations; testing owns check commands and isolation. Shelf core, protocol, service, native endpoint and client remain separate component references.

Completed plans retain decisions and screenshots, but do not define a new backlog. A pending gate in an old session means that session did not prove it. Link later evidence and the closed-scope decision rather than silently relabeling an old test as passed. Record a PR merge from local Git as such; it does not establish fresh hosted CI or native acceptance.

## Required Sections

Each guide includes Purpose, Key Files, Update Triggers and Related Docs. Add flows, limits, commands and prevention guidance where useful. Historical snapshots may retain their original layout beneath a clear notice; do not rewrite old counts to match a later suite.

## Conventions

- Use concise Markdown with short paragraphs and concrete paths/commands.
- Use relative Markdown links for navigation, including Related Docs; use code spans for source paths and commands.
- Use ASCII unless a document already needs non-ASCII content.
- Distinguish current source from published package behavior and exact build evidence.
- Keep secrets and private host paths out of docs. Reference protected configuration locations instead.

## Update Checklist

1. Update the owning current guide when behavior, commands, defaults or failure recovery change.
2. For a bug fix, record the root cause and the guardrail that prevents recurrence.
3. Put dated native/artifact results in history, preserving hashes, source identities, scope and unobserved limits. Link them from current guides rather than repeating the chronology.
4. When moving/merging a document, update inbound file/heading links and image paths. Preserve evidence and referenced images.
5. Add every document to the grouped index. Check local file/heading links, assets and `git diff --check`; verify documented commands against manifests/source without running mutating examples.

## Update Triggers

- Ownership, lifecycle, navigation or validation conventions change.
- Duplicate/stale status or a lost guardrail reveals an unclear documentation boundary.

## Related Docs

- [Docs index](index.md), [testing documentation checks](testing.md#documentation-checks).
- [Closed delivery scope](history/implementation-plan.md#future-work-and-verification-budget).
