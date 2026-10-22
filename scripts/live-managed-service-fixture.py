"""Bounded real user-systemd fixture. Refuses an existing managed unit.

Uses a temporary HOME/launcher/socket and only a new, verified-absent unit in the
user manager's runtime search path. Never changes persistent configuration or
stops/restarts the existing managed service.
"""
import json
import os
from pathlib import Path
import select
import shutil
import signal
import subprocess
import sys
import tempfile

UNIT = "ssh_clipboard-shelf.service"
CONTROL_ENV = None


def emit(value):
    print(json.dumps(value), flush=True)


def invoke(argv, env=None, success=True, timeout=45):
    result = subprocess.run([str(value) for value in argv], env=env,
                            capture_output=True, timeout=timeout)
    if success is not None and (result.returncode == 0) != success:
        raise RuntimeError(result.stderr.decode(errors="replace")[:2048])
    return result


def ctl(*args, success=True):
    return invoke(["systemctl", "--user", "--no-ask-password", "--no-pager", *args],
                  env=CONTROL_ENV, success=success)


def installed_identity():
    return ctl("show", "ssh_clipboard-shelf.service", "-p", "MainPID", "-p", "InvocationID",
               "-p", "ActiveState", "-p", "FragmentPath").stdout


def interrupt(*_):
    raise TimeoutError("Temporary service fixture was interrupted or expired")


def exercise(root, configuration_home):
    home = root / "home"
    home.mkdir(mode=0o700)
    store = root / "managed"
    socket = home / "custom %$ shelf.sock"
    env = dict(os.environ, HOME=str(home), XDG_CONFIG_HOME=str(configuration_home),
               XDG_DATA_HOME=str(home / "data"))
    packages = root / "packages"
    maintenance = root / "maintenance"
    shutil.copy(packages / "SSHClipboardServer-stable-linux-x86_64-0.3.0", maintenance)
    maintenance.chmod(0o700)
    def release(version):
        file = packages / f"SSHClipboardServer-stable-linux-x86_64-{version}"
        return ["--version", version, "--package", file, "--signature", str(file) + ".sig"]
    def command(action, *args, success=True):
        return invoke([maintenance, "server-maintenance", action, "--root", store, *args],
                      env=env, success=success, timeout=90)
    command("install", "--public-key", packages / "trusted.pub", *release("0.3.0"),
            "--socket-path", socket, "--max-items", "7", "--retention-secs", "42",
            "--acknowledge-clipboard-loss")
    assert os.readlink(store / "current") == "releases/0.3.0"
    assert ctl("is-active", UNIT).stdout.strip() == b"active"
    assert ctl("is-enabled", UNIT).stdout.strip() == b"enabled"
    (packages / "SSHClipboardServer-stable-linux-x86_64-0.3.0").unlink()
    saved = (store / "installation.json").read_bytes()
    command("update", *release("0.3.1"), "--acknowledge-clipboard-loss")
    assert os.readlink(store / "current") == "releases/0.3.1"
    assert (store / "installation.json").read_bytes() == saved
    # A long unit-specific rate-limit window guarantees the crashing release
    # leaves systemd start-limited when rollback begins, independent of defaults.
    dropin = configuration_home / "systemd/user" / f"{UNIT}.d"
    dropin.mkdir(mode=0o700)
    with (dropin / "fixture-limits.conf").open("x") as file:
        file.write("[Unit]\nStartLimitIntervalSec=60\nStartLimitBurst=3\n")
    ctl("daemon-reload")
    ctl("reset-failed", UNIT)
    failed = command("update", *release("0.3.2"), "--acknowledge-clipboard-loss", success=False)
    # The deliberately crashing release must roll back despite systemd rate limits.
    assert b"previous executable restored" in failed.stderr, failed.stderr.decode(errors="replace")
    assert os.readlink(store / "current") == "releases/0.3.1"
    assert ctl("is-active", UNIT).stdout.strip() == b"active"
    assert json.loads((store / "last-outcome.json").read_text()) == "restored_previous"
    assert not (store / "transition.json").exists()
    # Model interruption after selection, then exercise the real recovery CLI/service.
    journal = store / "transition.json"
    journal.write_text(json.dumps({"previous": "0.3.0", "next": "0.3.1"}))
    journal.chmod(0o600)
    command("recover", "--acknowledge-clipboard-loss")
    assert os.readlink(store / "current") == "releases/0.3.0"
    assert ctl("is-active", UNIT).stdout.strip() == b"active"
    emit({"passed": True, "checks": ["real user-systemd enable/start", "signed upgrade",
          "retained executable", "custom socket/options", "crash-loop rollback", "journal recovery"]})


def main():
    global CONTROL_ENV
    if os.geteuid() == 0:
        raise RuntimeError("Run this fixture as the intended non-root user")
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if not runtime or not Path(runtime).is_absolute():
        raise RuntimeError("An existing user-systemd runtime directory is required")
    configuration_home = Path(runtime)
    # Both the CLI and control operations target the actual manager's runtime
    # search path. The user's persistent ~/.config is neither edited nor chmodded.
    CONTROL_ENV = dict(os.environ, XDG_CONFIG_HOME=str(configuration_home))
    unit = configuration_home / "systemd/user" / UNIT
    dropin = unit.parent / f"{UNIT}.d"
    if os.path.lexists(unit) or os.path.lexists(dropin):
        raise RuntimeError("An existing managed unit is present; nothing changed")
    state = ctl("show", UNIT, "-p", "LoadState", "-p", "ActiveState", success=None).stdout
    if b"LoadState=not-found" not in state or b"ActiveState=inactive" not in state:
        raise RuntimeError("Managed unit name is already in use; nothing changed")
    if shutil.disk_usage("/tmp").free < 1024**3:
        raise RuntimeError("Fixture needs at least 1 GiB of temporary disk space")
    previous_identity = installed_identity()
    previous_route = shutil.which("ssh_clipboard")
    root = Path(tempfile.mkdtemp(prefix="ssh-clipboard-systemd-", dir="/tmp"))
    (root / "baseline.json").write_text(json.dumps({"installed": previous_identity.decode(), "ssh_route": previous_route}))
    signal.signal(signal.SIGALRM, interrupt)
    signal.signal(signal.SIGTERM, interrupt)
    signal.signal(signal.SIGINT, interrupt)
    signal.alarm(180)
    (root / "packages").mkdir(mode=0o700)
    emit({"root": str(root)})
    try:
        if not select.select([sys.stdin], [], [], 90)[0]:
            raise TimeoutError("No fixture command received")
        line = sys.stdin.readline()
        if not line or json.loads(line).get("command") == "stop":
            return
        if json.loads(line).get("command") != "start":
            raise RuntimeError("Unexpected fixture command")
        exercise(root, configuration_home)
    finally:
        signal.alarm(0)
        # Never remove a replacement unit or delete executables while still in use.
        if os.path.lexists(unit):
            if unit.is_symlink() or str(root / "home/.local/bin/ssh_clipboard") not in unit.read_text():
                raise RuntimeError(f"Unit ownership changed; retained fixture at {root}")
            if ctl("show", UNIT, "-p", "FragmentPath", "--value").stdout.decode().strip() != str(unit):
                raise RuntimeError(f"Loaded unit changed; retained fixture at {root}")
            ctl("stop", UNIT)
            ctl("reset-failed", UNIT, success=None)  # A stopped unit may already be unloaded.
            ctl("disable", UNIT)
            if ctl("show", UNIT, "-p", "ActiveState", "--value", success=None).stdout.strip() not in (b"", b"inactive"):
                raise RuntimeError(f"Test service did not stop; retained fixture at {root}")
            unit.unlink(missing_ok=True)
            if dropin.exists():
                limits = dropin / "fixture-limits.conf"
                if limits.is_symlink() or limits.read_text() != "[Unit]\nStartLimitIntervalSec=60\nStartLimitBurst=3\n":
                    raise RuntimeError(f"Fixture drop-in changed; retained data at {root}")
                limits.unlink()
                dropin.rmdir()
            ctl("daemon-reload")
        if installed_identity() != previous_identity or shutil.which("ssh_clipboard") != previous_route:
            raise RuntimeError(f"Existing service identity or SSH route changed; retained fixture at {root}")
        assert root.parent == Path("/tmp") and root.name.startswith("ssh-clipboard-systemd-")
        shutil.rmtree(root)
        emit({"cleaned": True, "installed_unchanged": True, "ssh_route_unchanged": True})


main()
