# Desktop 0.4.1 Acceptance

## Purpose

Record the signed desktop correction, real-profile Windows upgrade, startup setter acceptance and separately authorized public delivery. The Linux live transition remains separate.

## Key Files

- `desktop/src-tauri/src/startup_native.rs`: current-user Windows startup adapter and native argument-parser regression.
- `.github/workflows/desktop-release.yml`: protected exact-source draft builder.
- [0.4.0 acceptance](release-0.4.0-acceptance.md): original rework, production text round trip, legacy removal, actual login and isolated signed Linux service evidence.
- [Release readiness](release-readiness.md) and [release operations](releasing.md).

## Source and Signed Package

PR [#2](https://github.com/johansigfrids/ssh_clipboard/pull/2) merged at `80051adb78f9092e36562af4ac6de04fcace7715`. The merged tree matches reviewed head `9393ebe816824b43eb9ef621552b2460cd429b60`. [PR CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38898805874) and [master CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38900397686) each passed all seven jobs.

After owner approval, protected [desktop run 38900422933](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38900422933) built and signed the exact clean source as `desktop-v0.4.1`. Independent admission used retained public trust with SHA-256 `08c20604ca8fb74860ff1441426e3aa1d85f2ebf6acdbf44ef890d077bdcaab7`, key ID `08c20604ca8fb748`. No production private signing input was read by the assistant.

| Package | Bytes | SHA-256 |
| --- | ---: | --- |
| `SSHClipboard-stable-windows-x86_64-0.4.1-setup.exe` | 4860204 | `b1fd5eab5bf2a3fdb5f2887e74b856c421ca93a87f0d7e0823c57237de270453` |

The draft contained exactly the canonical installer, detached signature, checksum, keyed feed, candidate and build evidence. API asset digests/sizes, package checksum, clean release/source/platform/CLI/key evidence, candidate/feed agreement and independent signature/release-identity verification passed before execution. Installer ProductName is SSH Clipboard; ProductVersion and FileVersion are 0.4.1.

## Native Windows Upgrade

On 2026-10-16, the owner quit the 0.4.0 app, launched the verified installer from normal Windows Run, selected overwrite at the existing location, completed installation and confirmed that the reopened panel shows 0.4.1. The owner subsequently confirmed the saved server connection remains present.

Read-only Windows registry-service queries confirm the real uninstall registration is 0.4.1. `CIM_DataFile` reports both the installed GUI and bundled CLI file versions as 0.4.1.0, and process inspection finds one desktop process at the installed location. Direct automation filesystem reads and CLI execution still see isolated 0.4.0 files. Therefore the CLI claim here is real-file version metadata plus build-time CLI execution parity, not a real-profile post-install CLI execution test.

The owner confirmed Start at login remained enabled after upgrade, then disabled and re-enabled it. This control saves immediately. Read-only Windows registry-service queries verified removal followed by recreation of the startup entry, with the executable quoted independently of `--background`. StartupApproved is enabled and the old agent's Run entry is absent. The successful actual login recorded for 0.4.0 used this same installed path and command; no further sign-out was requested. No new production panel-flash observation is claimed.

Guardrail: automation-child registry and filesystem views can differ from the ordinary Windows desktop, even with an escalated tool. Human-launch installers and installed apps through Run/Explorer/Start, and verify native records through read-only Windows services. Never write through a provider to work around this boundary. Keep this environment issue separate from the valid startup quoting correction.

## Publication and Public Delivery

The acceptance record merged through PR [#3](https://github.com/johansigfrids/ssh_clipboard/pull/3) at `7548dadca900448c9f51e57277ee63663a49fdba`; [its CI](https://github.com/johansigfrids/ssh_clipboard/actions/runs/38905972681) passed all seven jobs and the merge tree matches the reviewed head. This documentation commit does not change package source `80051ad`.

After separate owner approval, [desktop-v0.4.1](https://github.com/johansigfrids/ssh_clipboard/releases/tag/desktop-v0.4.1) was published as the repository's latest release on 2026-10-16 at 13:19:33 UTC. The draft description and unsigned feed notes were prepared for the Windows scope before publication; only the feed's notes field changed, with its original retained locally. Installer, detached signature, checksum, candidate and build-evidence asset IDs/digests were unchanged.

Anonymous HTTPS requests verified the public latest-release API, repository-wide latest keyed feed, canonical installer, detached signature and checksum. The six public asset names/digests/sizes match the admitted draft; downloaded feed and package/signature/checksum bytes match the retained copies. Independent public-key package/release-identity admission passed again. This closes public Windows delivery, without claiming another native update installation or other platforms.

## Remaining Boundaries

- Desktop 0.4.1 is published and accepted for the bounded upgrade/startup and public-delivery checks. Retain the immutable 0.4.0 desktop draft as evidence; do not publish or overwrite it.
- The independent signed `server-v0.4.0` draft and existing Linux service are unchanged. Persistent-service/login/boot and the concrete legacy service/socket/SSH transition need their own review and authorization.
- Reuse the closed native history and installed updater/key-transition sessions. Interrupted native installation, omitted native recovery paths and macOS/Linux desktop installers retain their explicit gaps. No repetitive history, paste or sign-out session was added for this startup-only correction.
- Evidence-only follow-up commits do not change the source of either signed artifact.

## Update Triggers

- Publication/public delivery, Linux cutover or another exact-artifact acceptance boundary changes.
- A new corrective package replaces this candidate; never replace signed package bytes in place.

## Related Docs

- [Desktop updates](desktop-updates.md), [compatibility and cutover](compatibility-and-cutover.md), [server maintenance](server-maintenance.md).
- [Implementation plan](implementation-plan.md), [architecture](../ARCHITECTURE.md), [docs index](index.md).
