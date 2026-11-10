"""Tests for the ansible-lint entry point."""

import io
import os
import stat
import tempfile
import unittest
from unittest import mock

import ansible_lint_main


class AnsibleLintMainTest(unittest.TestCase):
    """Windows guard and shim layout."""

    def test_windows_guard(self):
        """Windows exits 69 with the actionable message."""
        err = io.StringIO()
        with mock.patch.object(os, "name", "nt"), mock.patch("sys.stderr", err):
            code = ansible_lint_main.main([])
        self.assertEqual(code, 69)
        self.assertIn("windows_x86_64 is unsupported", err.getvalue())

    def test_shims_on_path(self):
        """The four ansible shims are executable and PATH points at them."""
        with tempfile.TemporaryDirectory() as tmp:
            env = {"TMPDIR": tmp, "PATH": "/usr/bin"}
            shims = ansible_lint_main.prepare_env(env)
            self.assertEqual(env["PATH"], shims)
            for name in ansible_lint_main.SHIM_MODULES:
                mode = os.stat(os.path.join(shims, name)).st_mode
                self.assertTrue(mode & stat.S_IXUSR, name)
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

    def test_windows_guard_precedes_the_flag(self):
        """The Windows guard still exits 69 when the flag is present."""
        with (
            mock.patch.object(os, "name", "nt"),
            mock.patch("sys.stderr", io.StringIO()),
        ):
            code = ansible_lint_main.main(["--dx-collections-path", "/a"])
        self.assertEqual(code, 69)


if __name__ == "__main__":
    unittest.main()
