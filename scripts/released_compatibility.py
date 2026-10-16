"""Opt-in released-client probe on private Linux sockets; no SSH or OS clipboard."""
import argparse
from contextlib import ExitStack, contextmanager
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import signal
import stat
import struct
import subprocess
import sys
import tarfile
import tempfile
import time
import urllib.parse
import urllib.request
import zlib


RELEASE = "v0.3.0"
ASSET = "ssh_clipboard-linux-server.tar.gz"
URL = f"https://github.com/johansigfrids/ssh_clipboard/releases/download/{RELEASE}/{ASSET}"
# GitHub release asset 352117227, independently recorded before executing bytes.
ARCHIVE_SHA256 = "8a537d7b948c3a8f63312ddcf94286fbf0ac5fa03f326e47b2729307bde4f89c"
MAX_ARCHIVE = 16 * 1024 * 1024
MAX_BINARY = 64 * 1024 * 1024
LEGACY_LIMIT = 10 * 1024 * 1024
LEGACY_RESPONSE_OVERHEAD = 1024


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


class ReleaseRedirects(urllib.request.HTTPRedirectHandler):
    def __init__(self):
        self.count = 0

    def redirect_request(self, request, fp, code, message, headers, newurl):
        self.count += 1
        url = urllib.parse.urlsplit(newurl)
        require(self.count <= 5 and url.scheme == "https"
                and url.hostname in ("github.com", "release-assets.githubusercontent.com")
                and url.username is None and url.password is None
                and url.port in (None, 443), "Unexpected legacy release redirect.")
        return super().redirect_request(request, fp, code, message, headers, newurl)


def bounded_read(stream, maximum):
    result = bytearray()
    while chunk := stream.read(min(65536, maximum + 1 - len(result))):
        result.extend(chunk)
        require(len(result) <= maximum, "Legacy release input exceeds its size bound.")
    return bytes(result)


def download(destination):
    deadline = time.monotonic() + 60
    opener = urllib.request.build_opener(ReleaseRedirects())
    with opener.open(URL, timeout=10) as response:
        declared = response.headers.get("Content-Length")
        require(declared is None or 0 <= int(declared) <= MAX_ARCHIVE,
                "Oversized legacy release response.")
        result = bytearray()
        while chunk := response.read1(65536):
            require(time.monotonic() < deadline, "Legacy release download timed out.")
            result.extend(chunk)
            require(len(result) <= MAX_ARCHIVE, "Oversized legacy release response.")
    destination.write_bytes(result)


def verified_archive(archive):
    # Verify and extract the same captured bytes, even if a caller replaces the path.
    descriptor = os.open(archive, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, "rb") as source:
        require(stat.S_ISREG(os.fstat(source.fileno()).st_mode), "Use a regular legacy release archive.")
        data = bounded_read(source, MAX_ARCHIVE)
    require(hashlib.sha256(data).hexdigest() == ARCHIVE_SHA256,
            "Legacy release archive does not match the pinned v0.3.0 digest.")
    return data


def extract_binary(archive, destination):
    # Never extract archive-controlled paths, links, sparse files or permissions.
    found = False
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r|gz") as source:
        for member in source:
            require(not found and member.name == "ssh_clipboard" and member.isfile()
                    and not member.issparse() and 0 < member.size <= MAX_BINARY,
                    "Unexpected legacy archive member.")
            with source.extractfile(member) as contents:
                data = bounded_read(contents, MAX_BINARY)
            require(len(data) == member.size, "Incomplete legacy binary.")
            found = True
    require(found, "Legacy binary missing from archive.")
    with destination.open("xb") as output:
        output.write(data)
    destination.chmod(0o700)


def stop_process(process):
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=3)
    except subprocess.TimeoutExpired:
        pass
    finally:
        # A CLI can exit before its proxy child; kill the owned group, not only its leader.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=3)


def run(arguments, env, data=None, success=True):
    process = subprocess.Popen([str(arg) for arg in arguments], env=env,
                               stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, start_new_session=True)
    try:
        stdout, stderr = process.communicate(data, timeout=12)
        if success:
            require(process.returncode == 0,
                    f"Fixture command {Path(arguments[0]).name} {arguments[1]} failed: "
                    + stderr.decode("utf-8", errors="replace")[:500])
        return process.returncode, stdout
    except subprocess.TimeoutExpired as error:
        raise RuntimeError("A compatibility command exceeded its deadline.") from error
    finally:
        stop_process(process)
        for stream in (process.stdin, process.stdout, process.stderr):
            stream.close()


@contextmanager
def daemon(binary, command, sockets, env, extra=()):
    process = subprocess.Popen([str(binary), command, "--socket-path", str(sockets[0]), *extra],
                               env=env, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                               stderr=subprocess.DEVNULL, start_new_session=True)
    try:
        deadline = time.monotonic() + 8
        while True:
            require(process.poll() is None, "Disposable daemon exited during startup.")
            if all(socket.exists() and stat.S_ISSOCK(socket.stat().st_mode) for socket in sockets):
                break
            require(time.monotonic() < deadline, "Disposable sockets did not become ready.")
            time.sleep(0.02)
        yield
    finally:
        stop_process(process)


def shim(root, name, binary, endpoint, socket, allowed):
    script = root / name
    accepted = [["-T", "fixture-only", "ssh_clipboard", value] for value in allowed]
    script.write_text("#!" + sys.executable + "\nimport os,sys\n"
                      + f"if sys.argv[1:] not in {accepted!r}: sys.exit(64)\n"
                      + f"os.execv({str(binary)!r}, {[str(binary), endpoint, '--socket-path', str(socket)]!r})\n")
    script.chmod(0o700)
    return script


def png_fixture():
    def chunk(name, data):
        return struct.pack(">I", len(data)) + name + data + struct.pack(">I", zlib.crc32(name + data))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 2, 2, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(b"\0" + bytes([255, 0, 0, 255] * 2)
                                           + b"\0" + bytes([0, 255, 0, 128] * 2)))
            + chunk(b"IEND", b""))


def probe(root, current, legacy, env, checks):
    legacy_socket = root / "old" / "daemon.sock"
    shelf_socket = root / "new" / "shelf.sock"
    compat_socket = root / "new" / "daemon.sock"
    old_route = shim(root, "old-route", legacy, "proxy", legacy_socket, ("proxy", "session-proxy"))
    compat_route = shim(root, "compat-route", legacy, "proxy", compat_socket, ("proxy",))
    shelf_route = shim(root, "shelf-route", current, "session-proxy", shelf_socket, ("session-proxy",))

    def v2(binary, route, action, *args, data=None, success=True):
        return run([binary, action, "--target", "fixture-only", "--ssh-bin", route,
                    "--timeout-ms", "3000", *args], env, data, success)

    def shelf(action, *args, data=None, route=shelf_route, success=True):
        return run([current, "shelf", "--target", "fixture-only", "--ssh-bin", route,
                    "--timeout-ms", "3000", action, *args], env, data, success)

    def snapshot():
        return json.loads(shelf("list")[1])

    def share(data, format_="text"):
        shelf("share", "--stdin", "--format", format_, "--device-id",
              "00000000-0000-0000-0000-000000000001", "--device-name", "Fixture", data=data)

    with ExitStack() as stack:
        stack.enter_context(daemon(legacy, "daemon", [legacy_socket], env))
        stack.enter_context(daemon(current, "shelf-daemon", [shelf_socket, compat_socket], env,
                                   ("--legacy-socket-path", str(compat_socket))))
        text = "\n  Released client: åäö 🙂\r\nKeep trailing spaces  \n".encode()
        replacement = "New client: β\r\n ".encode()

        # Desktop-first: the new V2 client still works with the released daemon/proxy.
        v2(current, old_route, "push", "--stdin", data=text)
        out = root / "new-client-on-old.txt"
        v2(current, old_route, "pull", "--output", out)
        require(out.read_bytes() == text, "New client changed bytes from the released server.")
        v2(legacy, old_route, "push", "--stdin", data=replacement)
        out = root / "released-baseline.txt"
        v2(current, old_route, "pull", "--output", out)
        require(out.read_bytes() == replacement, "Released client baseline changed text.")
        checks.append("new V2 client and released daemon/proxy preserve exact text")

        code, _ = shelf("health", route=old_route, success=False)
        require(code != 0, "A fixed legacy proxy unexpectedly accepted the shelf protocol.")
        out = root / "after-refused-session.txt"
        v2(current, old_route, "pull", "--output", out)
        require(out.read_bytes() == replacement, "Refused session changed the old clipboard.")
        checks.append("fixed legacy proxy refuses session access without changing stored text")

        # Server-first: use the published CLI/proxy, not a reconstruction of its codec.
        code, _ = v2(legacy, compat_route, "peek", "--json", success=False)
        require(code == 2, "The new compatibility socket did not start empty.")
        v2(legacy, compat_route, "push", "--stdin", data=text)
        listed = snapshot()
        require(len(listed["items"]) == 1 and listed["items"][0]["origin"] is None,
                "Legacy-origin history metadata was not preserved.")
        item = listed["items"][0]["id"]
        require(shelf("fetch", item, "--stdout", "--epoch", listed["epoch"])[1] == text,
                "Shelf fetch changed the released client's text.")
        metadata = json.loads(v2(legacy, compat_route, "peek", "--json")[1])
        require(metadata["size"] == len(text), "Released PeekMeta size mismatch.")
        checks.append("released push/peek and new shelf fetch preserve exact Unicode/whitespace")

        share(replacement)
        out = root / "old-client-on-new.txt"
        v2(legacy, compat_route, "pull", "--output", out)
        require(out.read_bytes() == replacement, "Released pull changed new shelf text.")
        png = png_fixture()
        share(png, "png")
        out = root / "old-client-on-new.png"
        v2(legacy, compat_route, "pull", "--output", out)
        require(out.read_bytes() == png, "Released pull changed the exact PNG bytes.")
        checks.append("released pull reads newest shelf text and exact PNG bytes")

        listed = snapshot()
        require(len(listed["items"]) == 3, "Mixed clients did not share one history.")
        shelf("delete", listed["items"][0]["id"], "--epoch", listed["epoch"])
        out = root / "after-delete.txt"
        v2(legacy, compat_route, "pull", "--output", out)
        require(out.read_bytes() == replacement, "Legacy latest item did not follow deletion.")
        checks.append("new history deletion updates the released client's latest item")

        # The V2 receive limit deliberately includes a 1 KiB serialization allowance.
        near_limit = b"x" * (LEGACY_LIMIT + 64)
        share(near_limit)
        out = root / "within-response-allowance.txt"
        v2(legacy, compat_route, "pull", "--output", out)
        require(out.read_bytes() == near_limit, "Released response allowance changed bytes.")
        checks.append("released read preserves bytes within the 1 KiB response allowance")

        oversized = b"x" * (LEGACY_LIMIT + LEGACY_RESPONSE_OVERHEAD + 64)
        share(oversized)
        metadata = json.loads(v2(legacy, compat_route, "peek", "--json")[1])
        require(metadata["size"] == len(oversized), "Large-item metadata was unavailable.")
        out = root / "must-not-exist.txt"
        code, _ = v2(legacy, compat_route, "pull", "--output", out, success=False)
        require(code != 0 and not out.exists(), "Released client bypassed its transfer limit.")
        checks.append("released full frame budget refuses large Get while PeekMeta stays available")

        listed = snapshot()
        shelf("clear", "--epoch", listed["epoch"])
        code, _ = v2(legacy, compat_route, "peek", "--json", success=False)
        require(code == 2 and not snapshot()["items"], "Clear did not empty both views.")
        checks.append("new clear is visible as empty to the released client")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--current-binary", required=True, type=Path)
    parser.add_argument("--legacy-archive", type=Path)
    parser.add_argument("--evidence-file", required=True, type=Path)
    args = parser.parse_args()
    require(sys.platform == "linux" and platform.machine() == "x86_64", "Linux x86_64 required.")
    current = args.current_binary.resolve(strict=True)
    require(current.is_file() and os.access(current, os.X_OK), "Current headless executable required.")
    require(not args.evidence_file.exists(), "Choose a new evidence filename; never overwrite a run.")
    with current.open("rb") as binary:
        current_digest = hashlib.file_digest(binary, "sha256").hexdigest()
    report = {"legacy_release": RELEASE, "legacy_asset": ASSET,
              "legacy_archive_sha256": ARCHIVE_SHA256,
              "current_binary_sha256": current_digest,
              "platform": platform.platform(), "checks": [], "passed": False,
              "transport": "local fixed-route stdio shims; no SSH", "native_clipboard_tested": False,
              "installed_services_changed": False, "started_at": datetime.now(timezone.utc).isoformat()}
    revision = subprocess.run(["git", "-C", str(Path(__file__).resolve().parent.parent),
                               "rev-parse", "HEAD"], capture_output=True, text=True, timeout=5)
    report["current_source_commit"] = revision.stdout.strip() if revision.returncode == 0 else None
    try:
        with tempfile.TemporaryDirectory(prefix="ssh-clipboard-released-compat-", dir="/tmp") as directory:
            root = Path(directory)
            env = {**os.environ, "HOME": str(root / "home"), "XDG_CONFIG_HOME": str(root / "config"),
                   "XDG_DATA_HOME": str(root / "data"), "XDG_RUNTIME_DIR": str(root / "runtime"),
                   "TOKIO_WORKER_THREADS": "2", "RUST_LOG": "error"}
            for key in ("DISPLAY", "WAYLAND_DISPLAY", "SSH_AUTH_SOCK"):
                env.pop(key, None)
            for key in ("HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "XDG_RUNTIME_DIR"):
                Path(env[key]).mkdir(mode=0o700)
            archive = args.legacy_archive or root / ASSET
            if args.legacy_archive is None:
                download(archive)
            captured_archive = verified_archive(archive)
            legacy = root / "released-ssh_clipboard"
            extract_binary(captured_archive, legacy)
            report["legacy_binary_sha256"] = hashlib.sha256(legacy.read_bytes()).hexdigest()
            report["legacy_version"] = run([legacy, "--version"], env)[1].decode().strip()
            require(report["legacy_version"] == "ssh_clipboard 0.3.0", "Unexpected released version.")
            report["current_version"] = run([current, "--version"], env)[1].decode().strip()
            probe(root, current, legacy, env, report["checks"])
            report["passed"] = True
    except Exception as error:
        report["error"] = str(error)
    finally:
        report["fixture_cleaned"] = "root" in locals() and not root.exists()
        args.evidence_file.parent.mkdir(parents=True, exist_ok=True)
        with args.evidence_file.open("x") as output:
            json.dump(report, output, indent=2)
            output.write("\n")
    for check in report["checks"]:
        print("PASS: " + check)
    require(report["passed"] and report["fixture_cleaned"],
            report.get("error", "Fixture cleanup failed."))
    print("PASS: private daemons/proxies stopped and fixture directory removed")
    print("Evidence: " + str(args.evidence_file))


if __name__ == "__main__":
    main()
