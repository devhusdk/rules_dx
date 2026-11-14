"""Tests for the ansible-lint entry point."""

import multiprocessing
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import ansible_lint_main
import ansible_windows


class AnsibleLintMainTest(unittest.TestCase):
    """Windows guard and shim layout."""

    def test_windows_installs_posix_stand_ins(self):
        """Windows gets fcntl, termios, pwd and grp stand-ins and in-process ansible CLIs."""
        seen = []

        class RecordingPopen:
            def __init__(self, args, *_rest, **_kwargs):
                seen.append(list(args))

        with (
            mock.patch.dict(sys.modules),
            mock.patch.dict(os.environ, {}, clear=True),
            mock.patch.dict(os.__dict__),
            mock.patch.object(subprocess, "Popen", RecordingPopen),
            mock.patch.object(
                multiprocessing, "get_context", lambda method=None: method
            ),
        ):
            for name in ("fcntl", "termios", "pwd", "grp"):
                sys.modules.pop(name, None)
            os.__dict__.pop("getuid", None)
            ansible_windows.install(
                "/x/ansible_lint_main.py", ansible_lint_main.SHIM_MODULES
            )
            fcntl = sys.modules["fcntl"]
            self.assertIsNone(fcntl.flock(0, fcntl.LOCK_EX))
            self.assertEqual(fcntl.fcntl(0, fcntl.F_GETFD), 0)
            with self.assertRaises(OSError):
                fcntl.ioctl(1, 0)
            termios = sys.modules["termios"]
            with self.assertRaises(termios.error):
                termios.tcgetattr(0)
            self.assertEqual(termios.TIOCGWINSZ, 0)
            with self.assertRaises(KeyError):
                sys.modules["pwd"].getpwuid(0)
            with self.assertRaises(KeyError):
                sys.modules["grp"].getgrgid(0)
            self.assertEqual(os.getuid(), ansible_windows.POSIX_ID)
            self.assertEqual(multiprocessing.get_context("fork"), "spawn")
            self.assertEqual(os.environ["USERNAME"], "dx")
            popen = subprocess.Popen
            popen(["ansible-playbook", "--syntax-check", "site.yml"])
            popen(["/usr/bin/git", "status"])
        self.assertEqual(
            seen,
            [
                [
                    sys.executable,
                    "/x/ansible_lint_main.py",
                    "--ansible-shim",
                    "ansible-playbook",
                    "--syntax-check",
                    "site.yml",
                ],
                ["/usr/bin/git", "status"],
            ],
        )

    def test_shims_on_path(self):
        """The four ansible shims are executable and PATH points at them."""
        with tempfile.TemporaryDirectory() as tmp:
            env = {"TMPDIR": tmp, "PATH": "/usr/bin"}
            shims = ansible_lint_main.prepare_env(env)
            self.assertEqual(env["PATH"], shims)
            for name in ansible_lint_main.SHIM_MODULES:
                path = os.path.join(shims, name)
                self.assertTrue(os.path.isfile(path), name)
                if os.name != "nt":
                    self.assertTrue(os.stat(path).st_mode & stat.S_IXUSR, name)
            self.assertEqual(env["ANSIBLE_LINT_SKIP_SCHEMA_UPDATE"], "1")
            self.assertTrue(env["ANSIBLE_HOME"].startswith(tmp))

    def run_main(self, argv, inherited):
        """Runs main with a stub tool and returns (argv seen, env seen)."""
        seen = {}

        def entry():
            seen["argv"] = list(__import__("sys").argv)
            seen["env"] = dict(os.environ)
            return 0

        with tempfile.TemporaryDirectory() as tmp:
            env = dict(inherited, TMPDIR=tmp, PATH="/usr/bin")
            with (
                mock.patch.dict(os.environ, env, clear=True),
                mock.patch.object(ansible_lint_main, "import_tool", lambda *_: entry),
            ):
                self.assertEqual(ansible_lint_main.main(argv), 0)
        return seen["argv"], seen["env"]

    def test_collections_flag_is_removed_and_exported(self):
        """The leading flag becomes ANSIBLE_COLLECTIONS_PATH and never reaches ansible-lint."""
        argv, env = self.run_main(
            ["--dx-collections-path", "/a:/b", "--offline", "site.yml"],
            {"ANSIBLE_COLLECTIONS_PATH": "/host"},
        )
        self.assertEqual(argv, ["ansible-lint", "--offline", "site.yml"])
        self.assertEqual(env["ANSIBLE_COLLECTIONS_PATH"], "/a:/b")

    def test_inherited_collections_path_is_cleared_without_the_flag(self):
        """A host ANSIBLE_COLLECTIONS_PATH never leaks into the run."""
        argv, env = self.run_main(
            ["--offline", "site.yml"],
            {"ANSIBLE_COLLECTIONS_PATH": "/host", "ANSIBLE_COLLECTIONS_PATHS": "/host"},
        )
        self.assertEqual(argv, ["ansible-lint", "--offline", "site.yml"])
        self.assertNotIn("ANSIBLE_COLLECTIONS_PATH", env)
        self.assertNotIn("ANSIBLE_COLLECTIONS_PATHS", env)

    def test_windows_installs_stand_ins_before_the_flag(self):
        """Windows installs the stand-ins, then still honours the collections flag."""
        installed = []
        with (
            mock.patch.object(os, "name", "nt"),
            mock.patch.object(
                ansible_windows, "install", lambda *args: installed.append(args)
            ),
        ):
            argv, env = self.run_main(["--dx-collections-path", "/a", "site.yml"], {})
        self.assertEqual(len(installed), 1)
        self.assertEqual(installed[0][1], ansible_lint_main.SHIM_MODULES)
        self.assertEqual(argv, ["ansible-lint", "site.yml"])
        self.assertEqual(env["ANSIBLE_COLLECTIONS_PATH"], "/a")


if __name__ == "__main__":
    unittest.main()
