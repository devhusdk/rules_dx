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


if __name__ == "__main__":
    unittest.main()
