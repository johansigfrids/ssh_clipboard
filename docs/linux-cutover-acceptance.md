# Linux 0.4 Clean Cutover Acceptance

## Purpose

Record the owner-authorized live installation of the signed 0.4.0 server and retirement of the old installation, separately from server publication and a full reboot test.

## Key Files

- `src/server_management.rs`, `src/server_releases.rs`: managed launcher/unit, pinned options, verified activation and recovery.
- [0.4.0 artifact acceptance](release-0.4.0-acceptance.md): immutable source/package/trust and isolated service evidence.
- [Compatibility and cutover](compatibility-and-cutover.md): owner-requested clean replacement and return-path boundaries.

## Exact Artifact

On 2026-10-16, the owner authorized the clean Linux replacement, enabled user linger and completed the guarded privileged cleanup. The installed package is `SSHClipboardServer-stable-linux-x86_64-0.4.0`, 6854480 bytes, SHA-256 `05b5e3790b777bb8e3b9a02641671802b2a8c1cbe2678f86db9e146c5dad43ec`, built from `766f6e2659ecf984fdb568da10b8d941d07486d9`. Retained independent public trust is key ID `08c20604ca8fb748` with SHA-256 `08c20604ca8fb74860ff1441426e3aa1d85f2ebf6acdbf44ef890d077bdcaab7`.

Local package bytes/build identity/public trust and cryptographic admission were rechecked before transfer. Remote bytes and signature were rechecked before managed installation. The retained selected executable matches that admitted hash. No rebuild or signing-key change was needed for this deployment.

## Live Installation and Cleanup

The managed `ssh_clipboard-shelf.service` is persistently enabled and active at its retained immutable 0.4.0 executable. Selected-version protocol health passed. The stable user launcher resolves through the managed `current` link; fresh noninteractive SSH selects it after a silent user PATH block before the shell's interactive guard. Complete original shell bytes were backed up; SSH keys, restrictions and host verification were unchanged.

Five inspected user-owned directories had only group write removed, from 0775 to 0755: `.local`, `.local/share`, `.local/bin`, `.config/systemd`, and `.config/systemd/user`. No recursive permission/ownership operation was used. Managed and backup directories are private. The owner enabled linger so the user manager can remain available after logout and start at boot. This verifies configuration and persistent unit placement, not an observed server reboot or last-session logout.

The old daemon was stopped. `systemctl --user disable --now` also removed the old externally linked unit and enablement symlinks; after daemon-reload it is not found and inactive. The former user-local binary was replaced with the managed launcher. The owner ran a sudo helper that checked the new executable hash, active service, user route and exact old root-owned symlink before removing only the obsolete global launcher. Independent reads confirmed its absence.

After fresh SSH acceptance, final retirement rechecked the old extracted directory's owner/type/resolved path, exact three filenames, hashes and matching private backup copies. It removed only the old binary, downloaded archive and unit source, then removed the empty original directory. No old daemon/unit, extracted installation or old executable remains on the active routes. No Linux agent configuration/autostart file was found in the inspected project locations. These claims concern this installation, not an exhaustive audit of unrelated accounts or paths.

One private recovery directory outside active installation paths retains the original files, shell, unit-link metadata, mode/enablement baseline and operation receipts. Host-specific paths/controller receipts remain local and are not committed. Post-retirement return to the old installation requires restoring those copies and recreating its exact global symlink with sudo; it is not the managed updater's automatic rollback. Restoring executables cannot recover the lost in-memory clipboard. Linger remains the owner's explicit choice.

## Live Protocol Evidence

Both before and after final cleanup, fresh real SSH connections passed metadata health/history and exact Unicode/whitespace text Share/Fetch. A V2 Get returned the same latest bytes as the item-aware session. Only the validation item was deleted; the final history was empty. The new service retained its process/invocation across those management connections and cleanup. The validation used the previously admitted production 0.4.0 CLI, with explicit synthetic input/export; it did not read or write the Windows clipboard.

Pinned policy: 20 items, one-hour retention, 64 MiB encoded item, 256 MiB decoded/stored budgets, 384 MiB resident budget, 16 connections, 30-second I/O and five-minute idle timeout. The V2 compatibility frame cap remains 10 MiB. Legacy compatibility comes from the new shelf, with no second old daemon. Clipboard state remains memory-only.

## Server Publication

After separate owner approval, [server-v0.4.0](https://github.com/johansigfrids/ssh_clipboard/releases/tag/server-v0.4.0) was published on 2026-10-16 with `--latest=false`. Anonymous HTTPS downloads of all five canonical assets matched the admitted draft bytes, sizes and digests. Independent retained-key signature/release-identity verification passed for the public executable again. The repository's latest-release API still selects desktop-v0.4.1, and its public keyed feed is byte-for-byte unchanged. Publication did not reinstall or restart the service.

The [reviewed public verification key](security.md#signed-package-trust) is now documented for first installation. Immutable build evidence retains its original build-time `installed:false`/`published:false` fields; later deployment/publication does not rewrite signed package assets or their source.

## Remaining Boundaries

- The owner confirmed selecting Temporary history (new server) and saving the installed Windows 0.4.1 connection. This records the actual profile change; the completed native history/paste session was not repeated.
- Server 0.4.0 and desktop 0.4.1 are published; future server releases must also preserve the desktop keyed-feed latest selection. The superseded desktop-v0.4.0 stays a draft.
- Actual reboot/last-session logout, interrupted installation and previously omitted native recovery/platform checks retain their explicit gaps. No reboot or unrelated service change was performed.

## Prevention Guidance

Inspect old-install ownership, contents and references before retirement. Back up before stopping the old daemon, admit the exact signed replacement and fresh SSH route before cleanup, and keep recovery copies outside active paths. Systemd disable can remove the external unit symlink as well as enablement links; accept verified absence rather than treating that expected state as a migration failure. Never unlink an unexpected replacement, recursively delete a source checkout, widen an SSH key or infer public delivery from an installed draft.

## Update Triggers

- Desktop profile selection, server publication, reboot/logout or another live acceptance boundary changes.
- Installation paths/limits, cleanup ownership checks or rollback procedure change.

## Related Docs

- [Server maintenance](server-maintenance.md), [release readiness](release-readiness.md), [release operations](releasing.md).
- [Architecture](../ARCHITECTURE.md), [implementation plan](implementation-plan.md), [docs index](index.md).
