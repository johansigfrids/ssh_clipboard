# Managed Server Releases

## Purpose

Track Phase 7's signed package policy, per-user installation/update commands, and release recovery. The commands integrate bounded downloads, pinned trust, a stable launcher/unit, and user-systemd/proxy health. Production signing, real service acceptance, and reviewed legacy migration remain open.

## Key Files

- `src/server_releases.rs`: immutable staging, owned paths/lock, atomic selection, journal, and recovery.
- `src/server_package.rs`: bounded Minisign verification, authenticated version/platform/channel, and matching Linux ELF admission.
- `tests/fixtures/server-signing.json`: public synthetic signatures and nonexecutable ELF-header fixtures.
- `src/cli/server_maintenance.rs`, `tests/server_package_cli.rs`: explicit maintenance/verification commands and read-only process evidence.
- `src/server_management.rs`, `src/server_management/`: pinned installation configuration, bounded subprocesses/downloads, generated unit and proxy health.
- `desktop/scripts/smoke-server-maintenance.mjs`, `scripts/test-managed-server.py`: disposable signed binaries, fake systemctl, and real daemon/proxy lifecycle evidence.
- `src/cli/install_daemon.rs`: existing installer; its shared global link and extracted-directory unit remain unchanged.
- [Implementation plan](implementation-plan.md), [server setup](server-setup.md), and [release operations](releasing.md).

## Current Engine

The store requires an absolute user-owned installation directory with private release directories. It copies admitted executable bytes and a bounded receipt into a same-filesystem temporary directory, flushes files/directories, then publishes an immutable version directory. Deleting the original download cannot break the retained executable. Reusing a version with different admitted bytes is refused. Paths, symlinks, ownership, permissions, byte/record limits, and a persistent exclusive lock guard writes; the engine never invokes sudo or edits global installations.

Executable selection is one relative `current -> releases/<canonical-stable-version>` link replaced atomically. Normal activation requires a newer version and reverifies both selected and previous bytes through the caller's verifier before changing selection. A durable transition journal precedes the switch. A restart must pass the caller's **protocol** health check; process/service-active status alone is insufficient. Failure restores the previous executable, restarts, and rechecks it. Initial installation failure stops the attempted service and removes selection. Failed rollback or outcome persistence retains the journal and blocks new activation until recovery.

Recovery accepts only selection matching the journal's previous/next version, conservatively restores the verified previous release, and records the outcome. It cannot restore in-memory clipboard items lost on restart. The CLI must disclose that before activation. No automatic pruning of old versions or unfinished staging directories is implemented.

## Evidence and Remaining Integration

Eight WSL filesystem/service-adapter probes pass: retained bytes after source removal, version immutability/lock exclusion, rejected admission leaving current untouched, healthy selection with retained predecessor, failed restart/protocol health, interrupted switch and subsequent recovery, failed outcome write/rollback, and unsafe/tampered paths. The full headless suite also passes. These use a deliberately fake verifier and service; they neither prove server artifact cryptography nor exercise a real systemd installation. The desktop's cryptographic tests are separate evidence.

The concrete package policy now verifies exact bytes and the authenticated trusted-comment version/filename before admission. Names are `SSHClipboardServer-stable-linux-<arch>-<version>` for x86_64/aarch64; desktop packages or another channel/platform cannot be relabelled. Only after signature verification does a small ELF header check reject a wrong architecture/representation. Signatures/key text are bounded; packages cap at 256 MiB. The store reverifies on activation and rollback, while normal activation independently rejects version replay/downgrade. The owner has now chosen and configured the production signing identity; protected-build signature acceptance remains open.

Three additional WSL probes use real synthetic Minisign signatures to reject tampering, wrong keys, missing/unversioned signatures, forged versions, another channel/platform, and a signed invalid ELF header. One joins this policy to staging/activation and a failed-health rollback, without executing fixture bytes. Regenerate only server fixtures with `node desktop/scripts/generate-update-test-fixtures.mjs --server`; private synthetic keys are removed and desktop fixtures remain unchanged. The shared core uses the same locked verifier version as the desktop; existing dependency pins are preserved.

Linux exposes `ssh_clipboard server-maintenance verify --version <version> --public-key <independently-trusted.pub> --signature <package.sig> --package <package>`. It reads bounded regular files, selects the native architecture, verifies without executing bytes, and never changes a service/install. A real CLI process probe accepts the synthetic signed fixture and rejects forged versions, wrong keys, and altered bytes without changing any input file.

## Managed Commands

These commands require a user systemd manager and run as the target user. No sudo, global installation change, SSH-key change, or linger configuration is performed. The first key must be obtained independently; package-supplied keys cannot establish trust.

```sh
ssh_clipboard server-maintenance install --version <version> \
  --public-key <trusted.pub> --package <signed-binary> --signature <signed-binary.sig> \
  --acknowledge-clipboard-loss
ssh_clipboard server-maintenance status
ssh_clipboard server-maintenance update --version <newer-version> \
  --package <signed-binary> --signature <signed-binary.sig> --acknowledge-clipboard-loss
ssh_clipboard server-maintenance recover --acknowledge-clipboard-loss
```

Default storage is `$XDG_DATA_HOME/ssh_clipboard/server` (normally `~/.local/share/ssh_clipboard/server`); `--root <absolute-path>` selects another private installation. `installation.json` pins public trust, UID, exact socket paths and every daemon limit. Updates reuse those values; they do not accept a replacement key or options. `status` reports saved metadata/selection and recovery state, not live service health. Missing-installation status fails without creating it. Owned records reject symlinks/unsafe permissions; all local inputs reject nonregular/oversized files without blocking on FIFOs.

Install accepts all `shelf-daemon` options. It retains an immutable executable, links `~/.local/bin/ssh_clipboard` to `<root>/current/ssh_clipboard`, and writes an actual `ssh_clipboard-shelf.service` file under the user's systemd configuration directory. Existing conflicting launcher/unit files are refused. No unit points into a download folder. Literal paths are quoted with systemd environment substitution disabled and percent specifiers escaped; see [systemd execution rules](https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html). The selected signed binary must report the expected version before any service switch.

Omit **both** local package/signature flags to download the exact requested `server-v<version>` asset and `.sig` from this repository's GitHub Releases. No production server assets are available yet. Downloads use the existing `curl` command with default TLS validation, HTTPS-only redirects (maximum five), connect/total deadlines and byte limits. User curl configuration is disabled; an independent bounded pipe reader also caps streamed output. Signature reads cap at 16 KiB/20 seconds; packages at 256 MiB/300 seconds. No latest-version discovery or unattended restart is performed. See [curl options](https://curl.se/docs/manpage.html).

Every mutating command requires `--acknowledge-clipboard-loss` and discloses that restart/rollback loses the in-memory shelf. Initial service enablement occurs only after a durable transition journal and executable selection. Failed enable/start/health stops and disables a failed first installation; later update failure restores/restarts the retained predecessor. A failed requested update returns a failure exit even when rollback succeeds. Interrupted recovery can be retried while the journal remains. Protocol health runs the stable `session-proxy` against the saved socket and requires the selected server version, rather than accepting another healthy server or only checking systemd active state. Direct subprocesses have bounded output/deadlines and are killed/reaped on failure; no shell command strings are constructed.

The SSH command resolution must select this launcher: verify the noninteractive user's PATH, or use an explicitly reviewed absolute forced command. A custom socket also needs matching `session-proxy --socket-path` / compatibility `proxy --socket-path` arguments in that setup. Managed local health does not prove the SSH PATH/key route. A running legacy service or an occupied compatibility socket is never displaced automatically. Review [server setup](server-setup.md) before migration.

## Integrated Evidence and Remaining Gates

`node desktop/scripts/smoke-server-maintenance.mjs` uses the existing Windows Tauri signer and Ubuntu WSL Rust/Python/systemd tools. It builds isolated 0.3.0/0.3.1/0.3.2 headless source snapshots, signs actual binaries with a disposable key, then deletes the private key. All installations live under an owned temporary Linux HOME with a **fake systemctl**. Real daemon/proxy processes validate signed install/update, custom socket/limit preservation, executable survival after download removal, failed initial enable/restart cleanup, tampered/forged/replayed-package rejection before service calls, failed-restart rollback, and recovery from a modeled interrupted journal. The real `systemd-analyze --user verify` parses the generated unit. Temporary daemons, installations and signing files are cleaned up.

The complete WSL headless suite and all-target clippy pass; Windows agent clippy also passes. Independent tests fault child output/deadlines, unsafe/nonregular files and wrong-version protocol health. These results do **not** prove actual systemd enable/restart behavior, boot/login lifetime, real HTTPS asset downloads, native desktop use, or legacy migration. Test those on a disposable real user service before the existing Ubuntu service is changed. Signed staging and a protected draft workflow are implemented; [release operations](releasing.md#signed-server-drafts) records synthetic package evidence and production gates. The owner has configured production signing inputs, but their contents have not been read or exercised by a protected build. The user's current service has not been changed.

## Update Triggers

- Artifact/signature admission, storage layout, atomicity, or recovery behavior changes.
- CLI, service/proxy health integration, migration, or disposable/live evidence is added.

## Related Docs

- [Architecture](../ARCHITECTURE.md), [shelf server](shelf-server.md), and [testing](testing.md)
- [Server setup](server-setup.md), [releasing](releasing.md), and [docs index](index.md)
