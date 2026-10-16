"""Guard probes use synthetic archives only; the release integration is a separate opt-in run."""
import hashlib
import io
import os
from pathlib import Path
import sys
import subprocess
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch
import urllib.request

import released_compatibility as fixture


def archive(members):
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode="w:gz") as target:
        for name, kind, size in members:
            member = tarfile.TarInfo(name)
            member.type = kind
            data = b"controlled fixture"
            member.size = len(data) if size is None else size
            target.addfile(member, io.BytesIO(data) if member.isfile() and size is None else None)
    return output.getvalue()


@unittest.skipUnless(sys.platform == "linux", "Linux-only compatibility runner")
class ReleaseFixtureGuards(unittest.TestCase):
    def test_wrong_digest_never_admits_an_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "legacy.tar.gz"
            source.write_bytes(archive([("ssh_clipboard", tarfile.REGTYPE, None)]))
            with self.assertRaisesRegex(RuntimeError, "pinned v0.3.0 digest"):
                fixture.verified_archive(source)

    def test_nonregular_inputs_are_refused_without_blocking(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            fifo = root / "fifo"
            os.mkfifo(fifo)
            with self.assertRaisesRegex(RuntimeError, "regular"):
                fixture.verified_archive(fifo)
            link = root / "link"
            link.symlink_to(fifo)
            with self.assertRaises(OSError):
                fixture.verified_archive(link)

    def test_extraction_uses_the_verified_capture_even_if_the_source_changes(self):
        data = archive([("ssh_clipboard", tarfile.REGTYPE, None)])
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "legacy.tar.gz"
            destination = Path(directory) / "binary"
            source.write_bytes(data)
            with patch.object(fixture, "ARCHIVE_SHA256", hashlib.sha256(data).hexdigest()):
                captured = fixture.verified_archive(source)
            source.write_bytes(b"replaced after verification")
            fixture.extract_binary(captured, destination)
            self.assertEqual(destination.read_bytes(), b"controlled fixture")
            self.assertEqual(destination.stat().st_mode & 0o777, 0o700)

    def test_links_paths_duplicates_and_oversized_members_create_no_executable(self):
        cases = [
            [("../ssh_clipboard", tarfile.REGTYPE, None)],
            [("ssh_clipboard", tarfile.SYMTYPE, 0)],
            [("ssh_clipboard", tarfile.LNKTYPE, 0)],
            [("ssh_clipboard", tarfile.REGTYPE, fixture.MAX_BINARY + 1)],
            [("ssh_clipboard", tarfile.REGTYPE, None)] * 2,
            [],
        ]
        for members in cases:
            with self.subTest(members=members), tempfile.TemporaryDirectory() as directory:
                destination = Path(directory) / "binary"
                with self.assertRaises(RuntimeError):
                    fixture.extract_binary(archive(members), destination)
                self.assertFalse(destination.exists())

    def test_read_bound_does_not_accept_an_extra_byte(self):
        self.assertEqual(fixture.bounded_read(io.BytesIO(b"1234"), 4), b"1234")
        with self.assertRaisesRegex(RuntimeError, "size bound"):
            fixture.bounded_read(io.BytesIO(b"12345"), 4)

    def test_redirects_cannot_downgrade_tls_or_leave_release_hosts(self):
        for url in ("http://github.com/asset", "https://example.com/asset",
                    "https://user@github.com/asset", "https://github.com:444/asset"):
            with self.subTest(url=url), self.assertRaisesRegex(RuntimeError, "redirect"):
                fixture.ReleaseRedirects().redirect_request(
                    urllib.request.Request(fixture.URL), None, 302, "Found", {}, url)
        redirects = fixture.ReleaseRedirects()
        redirects.count = 5
        with self.assertRaisesRegex(RuntimeError, "redirect"):
            redirects.redirect_request(urllib.request.Request(fixture.URL), None, 302,
                                       "Found", {}, "https://github.com/asset")

    def test_cleanup_stops_a_proxy_even_after_its_cli_leader_exits(self):
        script = ("import os,time\nchild=os.fork()\n"
                  "if child == 0:\n time.sleep(30)\n os._exit(0)\n"
                  "print(child,flush=True)\nos._exit(0)\n")
        process = subprocess.Popen([sys.executable, "-c", script], stdout=subprocess.PIPE,
                                   start_new_session=True)
        try:
            child = int(process.stdout.readline())
            process.wait(timeout=3)
            fixture.stop_process(process)
            deadline = time.monotonic() + 3
            while True:
                status = Path(f"/proc/{child}/stat")
                if not status.exists() or status.read_text().split()[2] == "Z":
                    break
                self.assertLess(time.monotonic(), deadline, "Owned proxy survived cleanup")
                time.sleep(0.01)
        finally:
            fixture.stop_process(process)
            process.stdout.close()


if __name__ == "__main__":
    unittest.main()
