> Historical 0.4 compatibility guide. The 0.5 source removes these implementations; use the current [fresh setup guide](../../compatibility-and-cutover.md).

# Released Client Compatibility

## Purpose

Test the actual published legacy executable against the rework, independently of current-source V2 fixtures. This covers Linux CLI/proxy/daemon protocol migration; it does not install a desktop or replace a server.

## Key Files

- `scripts/released_compatibility.py`: pinned release admission, private fixture lifecycle and old/new process exchanges.
- `scripts/test_released_compatibility.py`: synthetic archive, redirect and read-bound guard probes.
- `.github/workflows/ci.yml`: Linux headless job runs both probes after its release build and retains JSON evidence.
- [Compatibility/cutover](compatibility-and-cutover.md): connection matrix and separately authorized installation transition.

## Baseline and Trust

The baseline is the published `v0.3.0` Linux x86_64 server archive, which contains the CLI and both `daemon`/`proxy` commands. The probe downloads only its exact GitHub release URL, with HTTPS/allowed-host redirect checks, five redirects at most, bounded reads and timeouts. `--legacy-archive <file>` can reuse a local copy under the same admission policy.

The archive's pinned SHA-256 is `8a537d7b948c3a8f63312ddcf94286fbf0ac5fa03f326e47b2729307bde4f89c`, independently recorded from GitHub release asset `352117227`. The extracted executable reports `ssh_clipboard 0.3.0`; its SHA-256 is `7a55638b1f15ff133a088d6728aa1e25e7fe8911ac1ef15bb5ccc30b732fca1e`. This historical release has no updater signature. HTTPS plus a reviewed archive pin establishes this test baseline; it does not waive production package signing or first-install trust.

Verify the bounded regular-file archive before parsing it and extract from those same captured bytes. Accept exactly one regular, nonsparse `ssh_clipboard` member, never archive-controlled destinations, links or modes. Validate all members before creating the executable. The extracted file is private and executable only inside the owned fixture. Never update the expected digest automatically after a failed download.

## Run

With existing Rust/Python 3.11+ tools on Linux x86_64:

```sh
cargo build --locked --no-default-features
python3 -m unittest discover -s scripts -p test_released_compatibility.py -v
python3 scripts/released_compatibility.py --current-binary target/debug/ssh_clipboard --evidence-file desktop/artifacts/released-compatibility-run.json
```

Choose a fresh evidence filename for every run; existing reports are never overwritten. On this Windows workspace, use the existing Ubuntu WSL toolchain from the repository directory:

```powershell
wsl.exe -d Ubuntu -- bash -lc 'cargo build --locked --no-default-features --target-dir target/linux-wsl'
wsl.exe -d Ubuntu --exec python3 -m unittest discover -s scripts -p test_released_compatibility.py -v
wsl.exe -d Ubuntu --exec python3 scripts/released_compatibility.py --current-binary target/linux-wsl/debug/ssh_clipboard --evidence-file desktop/artifacts/released-compatibility-run.json
```

CI instead selects its freshly built optimized `target/release/ssh_clipboard`, runs the same guard/compatibility tests in the Linux server job and retains the metadata-only report for 14 days. Other jobs do not download or execute this Linux baseline.

## Observable Contracts

- Desktop-first protocol route: the new V2 CLI pushes/pulls exact Unicode, whitespace and line endings through the released proxy/daemon.
- A fixed released `proxy` refuses a new session request; its stored text remains unchanged afterward. The fixture simulates a fixed forced-command route locally; this does not change or verify an actual SSH key.
- Server-first protocol route: released CLI Push/PeekMeta through the released proxy reaches the new shelf's explicit V2 socket. New-session List/Fetch sees the same exact bytes and unknown legacy origin.
- Released Pull reads the newest shelf text and exact PNG bytes produced by new-session Share. No native clipboard or GUI image decode is exercised.
- History deletion changes the released client's latest item, and Clear produces an empty result in both views.
- Legacy limits remain explicit: the nominal 10 MiB receive budget includes a 1 KiB response/serialization allowance. A synthetic item inside that allowance is readable; one beyond the full frame budget is refused without creating the output file, while PeekMeta remains available.

## Isolation and Evidence

Every run creates a private `/tmp/ssh-clipboard-released-compat-*` tree with separate HOME/XDG directories, owned sockets and synthetic data. It launches two disposable real daemon processes. Local executable shims accept only the exact fixture invocation and exec a fixed owned proxy/socket; they never start SSH, access user settings or use the OS clipboard. No systemd unit, installed binary, startup entry or SSH configuration is changed.

Commands have deadlines and owned process groups. Cleanup terminates the entire group, including a proxy left after its CLI exits, waits for direct children and removes the temporary tree. The report records release/archive/binary hashes, current source/version/platform, completed checks, pass/failure and confirmed directory cleanup. Failed runs retain a separate report, not private fixture contents. The script does not add persistent services or signing material.

On 2026-10-16, all seven synthetic guard tests and all eight real released-binary checks passed in Ubuntu WSL (glibc 2.39), against current runtime source `3db238c`. Cleanup passed. The first run exposed an incorrect refusal threshold in the probe: it ignored `RESPONSE_OVERHEAD = 1024` and treated a valid response as oversized. The corrected probe tests both the allowance and refusal beyond it; no production limit was changed.

This establishes the tested Linux x86_64 `v0.3.0` protocol pairing. It does not establish every older release/platform, a support-window end date, an installed migration, real SSH forced-command authorization, desktop paste or a production service cutover. Existing real SSH and installed Windows results remain separate evidence.

## Update Triggers

- Legacy bootstrap/codec, response allowance, compatibility adapter or new-session behavior changes.
- Another released baseline/platform is deliberately accepted after independent asset review.
- Actual installed-client/service migration adds evidence beyond local process routes.

## Related Docs

- [Compatibility and cutover](compatibility-and-cutover.md), [testing](../../testing.md), and [CI](../../ci.md)
- [Shelf server](../../shelf-server.md), [session protocol](../../session-protocol.md), and [implementation plan](../implementation-plan.md)
