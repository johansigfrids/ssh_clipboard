"""Owned disposable SSH test fixture; no installed services or user clipboard access."""
import json
import os
import select
from pathlib import Path
import subprocess
import sys
import tempfile
import time


def emit(value):
    print(json.dumps(value), flush=True)


with tempfile.TemporaryDirectory(prefix="ssh-clipboard-integration-", dir="/tmp") as directory:
    root = Path(directory)
    binary = root / "ssh_clipboard"
    socket = root / "runtime" / "shelf.sock"
    server = None
    emit({"root": str(root)})
    try:
        lifetime = time.monotonic() + 120
        while True:
            remaining = lifetime - time.monotonic()
            if remaining <= 0 or not select.select([sys.stdin], [], [], remaining)[0]:
                break
            line = sys.stdin.readline()
            if not line:
                break
            command = json.loads(line)["command"]
            if command == "stop":
                break
            if command != "start" or server is not None:
                raise RuntimeError("invalid fixture command")
            os.chmod(binary, 0o700)
            server = subprocess.Popen(
                [str(binary), "shelf-daemon", "--socket-path", str(socket)],
                stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL, env={**os.environ, "TOKIO_WORKER_THREADS": "2"},
            )
            deadline = time.monotonic() + 8
            while not socket.exists() and server.poll() is None and time.monotonic() < deadline:
                time.sleep(0.02)
            emit({"ready": socket.exists() and server.poll() is None, "exit_code": server.poll()})
    finally:
        if server is not None:
            server.terminate()
            try:
                server.wait(timeout=3)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait(timeout=3)
emit({"stopped": True})
