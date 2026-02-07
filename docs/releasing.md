# Releasing

## Purpose
Describe how to cut and publish releases, validate artifacts, and roll back if needed.

## Key Files
- `.github/workflows/release.yml`
- `Cargo.toml`
- `CHANGELOG.md` (if used)

## Release Steps
1. Ensure `master` is green in CI.
2. Update docs for the release:
   - Update `CHANGELOG.md` with the release notes.
   - Verify `README.md` is accurate for installation and the common setup.
3. Verify `.github/workflows/release.yml` is up to date with current release outputs:
   - Build matrix entries (platforms/targets and agent vs server variants).
   - Artifact names and packaging format (`.zip`/`.tar.gz`) match docs and expected release assets.
   - Added/removed binaries are reflected in packaging commands.
4. Bump the version in `Cargo.toml` and commit the change.
5. Decide the next version (e.g., `0.2.0`) and tag with `v` prefix:
   - `git tag v0.2.0`
   - `git push origin v0.2.0`
6. GitHub Actions runs `release.yml` and publishes artifacts + checksums.

## Validate Artifacts
- Download the artifacts from the GitHub Release page.
- macOS publishes two agent artifacts:
  - `ssh_clipboard-macos-x86_64-agent` (Intel)
  - `ssh_clipboard-macos-aarch64-agent` (Apple Silicon)
- Smoke test:
  - `ssh_clipboard --help`
  - `ssh_clipboard proxy --help`
  - `ssh_clipboard daemon --help` (Linux)
  - `ssh_clipboard agent --help` (Windows/macOS/Linux agent builds)

## Rollback
- If a release is bad, delete the GitHub Release and tag:
  - `git tag -d v0.2.0`
  - `git push origin :refs/tags/v0.2.0`
- Fix the issue and tag a new patch release.

## Update Triggers
- Changes to release packaging, platforms, or artifact naming.
- Changes to versioning strategy or CI release triggers.

## Related Docs
- `docs/ci.md`
- `docs/testing.md`
