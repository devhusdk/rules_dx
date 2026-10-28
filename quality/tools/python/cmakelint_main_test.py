"""Tests for the cmakelint entry point."""

import os
import subprocess
import sys
import tempfile
import unittest

import cmakelint_main

CLEAN = "cmake_minimum_required(VERSION 3.20)\n"
DIRTY = "cmake_minimum_required(VERSION 3.20)\nSET(A b)\n"


def run(directory, name, text, *flags):
    """Runs the entry point on one file and returns (code, stdout, stderr)."""
    path = os.path.join(directory, name)
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)
    code = (
        "import sys, cmakelint_main\n"
        "sys.argv = ['cmakelint'] + sys.argv[1:]\n"
        "sys.exit(cmakelint_main.main())\n"
    )
    proc = subprocess.run(
        [sys.executable, "-c", code, "--config=None", *flags, "--", name],
        cwd=directory,
        env={
            **os.environ,
            "PYTHONPATH": os.pathsep.join(sys.path),
        },
        capture_output=True,
        check=False,
    )
    return (
        proc.returncode,
        proc.stdout.decode("utf-8"),
        proc.stderr.decode("utf-8"),
    )


class CmakelintMainTest(unittest.TestCase):
    """Exit codes and encoding of the cmakelint wrapper."""

    def test_utf8_open_is_strict_utf8(self):
        """The shim decodes UTF-8 regardless of the locale."""
        with tempfile.TemporaryDirectory() as directory:
            path = os.path.join(directory, "a.txt")
            with open(path, "wb") as handle:
                handle.write("é".encode("utf-8"))
            with cmakelint_main.utf8_open(path) as handle:
                self.assertEqual(handle.read(), "é")
            with open(path, "wb") as handle:
                handle.write(b"\xff")
            with self.assertRaises(UnicodeDecodeError):
                with cmakelint_main.utf8_open(path) as handle:
                    handle.read()

    def test_clean_file_exits_zero(self):
        """A clean file exits 0 and reports a zero total."""
        with tempfile.TemporaryDirectory() as directory:
            code, out, err = run(directory, "CMakeLists.txt", CLEAN)
        self.assertEqual((code, out), (0, ""))
        self.assertIn("Total Errors: 0", err)

    def test_findings_exit_one(self):
        """A dirty file exits 1 with path:line findings."""
        with tempfile.TemporaryDirectory() as directory:
            code, out, err = run(directory, "CMakeLists.txt", DIRTY)
        self.assertEqual(code, 1)
        self.assertIn("CMakeLists.txt:2:", out)
        self.assertIn("Total Errors: 1", err)

    def test_usage_error_exits_32(self):
        """A bad flag value is a usage error."""
        with tempfile.TemporaryDirectory() as directory:
            code, _, _ = run(directory, "CMakeLists.txt", CLEAN, "--spaces=x")
        self.assertEqual(code, 32)

    def test_non_ascii_utf8_file(self):
        """A UTF-8 file with non-ASCII comments is read without a decode error."""
        with tempfile.TemporaryDirectory() as directory:
            code, out, _ = run(
                directory, "CMakeLists.txt", "# café ☃\n" + CLEAN
            )
        self.assertEqual((code, out), (0, ""))


if __name__ == "__main__":
    unittest.main()
