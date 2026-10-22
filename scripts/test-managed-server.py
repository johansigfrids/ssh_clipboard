"""Disposable Linux maintenance evidence; fake systemctl, real signed binaries and proxy/daemon.

Invoked by desktop/scripts/smoke-server-maintenance.mjs. Never calls the real systemctl.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[1]
VERSIONS = ("0.3.0", "0.3.1", "0.3.2")


def execute(argv, *, env=None, success=True, timeout=60):
    result = subprocess.run([str(arg) for arg in argv], env=env, capture_output=True, timeout=timeout)
    if (result.returncode == 0) != success:
        raise AssertionError(f"Unexpected command outcome: {result.stderr.decode(errors='replace')}")
    return result


def prepare(output, crashing_release=False):
    with tempfile.TemporaryDirectory(prefix="ssh-clipboard-build-") as temporary:
        source = Path(temporary)
        shutil.copytree(REPO / "src", source / "src")
        shutil.copy(REPO / "build.rs", source / "build.rs")
        manifest = (REPO / "Cargo.toml").read_text()
        lock = (REPO / "Cargo.lock").read_text()
        current = manifest.split('version = "', 1)[1].split('"', 1)[0]
        daemon_source = source / "src/shelf_daemon.rs"
        original_daemon = daemon_source.read_text()
        for version in VERSIONS:
            # Deliberately bad signed test release: --version works, daemon exits.
            # Only the disposable source snapshot is changed, never repository source.
            if crashing_release and version == VERSIONS[-1]:
                needle = "    let service = Arc::new(ShelfService::new(limits, policy)?);"
                assert original_daemon.count(needle) == 1
                daemon_source.write_text(original_daemon.replace(needle,
                    '    if std::hint::black_box(true) { return Err(eyre::eyre!("synthetic test daemon crash")); }\n' + needle))
            else:
                daemon_source.write_text(original_daemon)
            (source / "Cargo.toml").write_text(manifest.replace(f'version = "{current}"', f'version = "{version}"', 1))
            (source / "Cargo.lock").write_text(lock.replace(f'name = "ssh_clipboard"\nversion = "{current}"', f'name = "ssh_clipboard"\nversion = "{version}"', 1))
            execute([Path.home() / ".cargo/bin/cargo", "build", "--locked",
                     "--manifest-path", source / "Cargo.toml", "--target-dir", REPO / "target/linux-maintenance-smoke",
                     "--bin", "ssh_clipboard"], timeout=600)
            shutil.copy(REPO / "target/linux-maintenance-smoke/debug/ssh_clipboard",
                        output / f"SSHClipboardServer-stable-linux-x86_64-{version}")


FAKE_SYSTEMCTL = r'''#!/usr/bin/python3
import json,os,signal,subprocess,sys,time
from pathlib import Path
home=Path(os.environ['HOME'])
root=home/'managed'
pidfile=home/'daemon.pid'
args=[a for a in sys.argv[1:] if not a.startswith('--')]
action=args[0]
with (home/'calls').open('a') as log: log.write(action+'\n')
if action=='enable': (home/'enabled').touch()
if action=='enable' and (home/'fail-enable').exists():
    (home/'fail-enable').unlink();sys.exit(1)
if action=='disable': (home/'enabled').unlink(missing_ok=True)
if action=='is-failed': sys.exit(0 if (home/'failed').exists() else 1)
if action=='reset-failed': (home/'failed').unlink(missing_ok=True)
if action=='restart' and (home/'fail-restart').exists():
    (home/'fail-restart').unlink();(home/'failed').touch();sys.exit(1)
if action in ('restart','stop'):
    if pidfile.exists():
        pid=int(pidfile.read_text())
        try: os.kill(pid,signal.SIGTERM)
        except ProcessLookupError: pass
        for _ in range(200):
            stat=Path(f'/proc/{pid}/stat')
            if not stat.exists() or stat.read_text().split()[2]=='Z': break
            time.sleep(.01)
        else: raise RuntimeError('fixture daemon did not stop')
        pidfile.unlink()
if action=='restart':
    config=json.loads((root/'installation.json').read_text())
    d=config['daemon'];argv=[config['launcher'],'shelf-daemon']
    for key,value in d.items():
        if value is not None: argv.extend(['--'+key.replace('_','-'),str(value)])
    with (home/'daemon.log').open('ab') as log:
        child=subprocess.Popen(argv,stdin=subprocess.DEVNULL,stdout=log,stderr=log,close_fds=True,start_new_session=True)
    pidfile.write_text(str(child.pid))
if action not in ('enable','disable','restart','stop','daemon-reload','reset-failed'): sys.exit(2)
'''


def test(packages):
    with tempfile.TemporaryDirectory(prefix="ssh-clipboard-managed-") as temporary:
        home = Path(temporary)
        tools = home / "tools"
        tools.mkdir()
        fake = tools / "systemctl"
        fake.write_text(FAKE_SYSTEMCTL)
        fake.chmod(0o700)
        env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(home / "config"),
                   XDG_DATA_HOME=str(home / "data"), PATH=f"{tools}:/usr/bin:/bin")
        root = home / "managed"
        # Execute an independent maintenance binary, so removal of download inputs is real.
        binary = home / "maintenance"
        shutil.copy(packages / f"SSHClipboardServer-stable-linux-x86_64-{VERSIONS[0]}", binary)
        binary.chmod(0o700)
        socket = home / 'custom %$ shelf.sock'
        def release(version):
            package = packages / f"SSHClipboardServer-stable-linux-x86_64-{version}"
            return ["--version", version, "--package", package, "--signature", str(package)+".sig"]
        def command(action, *arguments, success=True):
            return execute([binary, "server-maintenance", action, "--root", root, *arguments], env=env, success=success)
        def current():
            return os.readlink(root / "current")
        def calls():
            return (home / "calls").read_bytes() if (home / "calls").exists() else b""
        try:
            command("status", success=False)
            assert not root.exists()
            install = ["--public-key", packages / "trusted.pub", *release(VERSIONS[0]),
                       "--socket-path", socket, "--max-items", "7", "--retention-secs", "42"]
            command("install", *install, success=False)  # Explicit loss acknowledgment required.
            assert not root.exists() and not calls()
            (home / "fail-enable").touch()
            command("install", *install, "--acknowledge-clipboard-loss", success=False)
            assert not (root / "current").exists() and not (home / "enabled").exists()
            assert not (root / "transition.json").exists()
            (home / "fail-restart").touch()
            command("install", *install, "--acknowledge-clipboard-loss", success=False)
            assert not (root / "current").exists() and not (home / "enabled").exists()
            command("install", *install, "--acknowledge-clipboard-loss")
            assert current() == "releases/0.3.0" and (home / "enabled").exists()
            config = (root / "installation.json").read_bytes()
            unit = home / "config/systemd/user/ssh_clipboard-shelf.service"
            unit_bytes = unit.read_bytes()
            # Parse through systemd's own unit verifier without starting/installing any unit.
            execute(["systemd-analyze", "--user", "verify", unit], env=env)
            (packages / f"SSHClipboardServer-stable-linux-x86_64-{VERSIONS[0]}").unlink()
            assert (home / ".local/bin/ssh_clipboard").resolve().is_file()
            before = calls()
            new_package = packages / f"SSHClipboardServer-stable-linux-x86_64-{VERSIONS[1]}"
            original = new_package.read_bytes()
            new_package.write_bytes(original + b"tampered")
            command("update", *release(VERSIONS[1]), "--acknowledge-clipboard-loss", success=False)
            new_package.write_bytes(original)
            assert calls() == before and current() == "releases/0.3.0"
            # A signed fixture whose requested version was forged cannot reach systemctl.
            forged = release(VERSIONS[1]); forged[1] = "9.0.0"
            command("update", *forged, "--acknowledge-clipboard-loss", success=False)
            assert calls() == before and current() == "releases/0.3.0"
            command("update", *release(VERSIONS[1]), "--acknowledge-clipboard-loss")
            assert current() == "releases/0.3.1"
            assert (root / "installation.json").read_bytes() == config and unit.read_bytes() == unit_bytes
            before = calls()
            command("update", *release(VERSIONS[1]), "--acknowledge-clipboard-loss", success=False)
            assert calls() == before  # Replay refusal precedes any service mutation.
            (home / "fail-restart").touch()
            failed = command("update", *release(VERSIONS[2]), "--acknowledge-clipboard-loss", success=False)
            assert b"previous executable restored" in failed.stderr and current() == "releases/0.3.1"
            assert json.loads((root / "last-outcome.json").read_text()) == "restored_previous"
            assert not (root / "transition.json").exists()
            # Model a crash after selection: recovery conservatively restores retained old bytes.
            journal = root / "transition.json"
            journal.write_text(json.dumps({"previous": "0.3.0", "next": "0.3.1"}))
            journal.chmod(0o600)
            command("update", *release(VERSIONS[2]), "--acknowledge-clipboard-loss", success=False)
            command("recover", "--acknowledge-clipboard-loss")
            assert current() == "releases/0.3.0" and not journal.exists()
            status = json.loads(command("status").stdout)
            assert status["selected_version"] == "0.3.0" and not status["recovery_required"]
            print("PASS: signed install/update, failed initial enable/restart cleanup, custom options, retained executable, systemd unit grammar, tampered/forged/replayed package rejection, real proxy health, failed-restart rollback and journal recovery.")
            print("Evidence: fake systemctl + real headless daemon/proxy; no actual user service or live host was modified.")
        finally:
            # Use only our fake controller with our disposable environment.
            execute([fake, "--user", "stop", "ssh_clipboard-shelf.service"], env=env)


if __name__ == "__main__":
    mode, destination, *options = sys.argv[1:]
    if mode == "prepare":
        prepare(Path(destination).resolve(), crashing_release="--crashing-release" in options)
    else:
        {"test": test}[mode](Path(destination).resolve())
