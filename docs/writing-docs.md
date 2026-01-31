# Writing Docs

## Purpose
Define how to author and maintain internal documentation for this repo.

## Key Files
- `docs/index.md`
- `AGENTS.md`

## Required Sections
Each doc should include:
- `Purpose`: one or two sentences.
- `Key Files`: relevant paths.
- `Update Triggers`: when to revisit the doc.
- `Related Docs`: cross-links.

Add other sections as needed (flows, settings, pitfalls, examples).

## Conventions
- Use concise Markdown with short paragraphs and lists.
- Keep content internal; avoid end-user marketing language.
- Use ASCII characters unless a file already uses non-ASCII.
- Link to other docs with relative paths.

## Update Checklist
- Code changes: update the relevant feature doc(s) and add/refresh links in `docs/index.md`.
- Bug fixes: treat as a doc signal; capture the edge case or error path.
- New docs: add to `docs/index.md` and verify cross-links.

## Update Triggers
- When knowledge should have been preserved but was lost.
- When doc formatting is unclear or inconsistent.

## Related Docs
- `docs/index.md`
