"""Covers the argument parsing and exit code of the griffe check driver."""

import os
import sys
import unittest
from unittest import mock

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import griffe_check


class GriffeCheckTestCase(unittest.TestCase):
    """Runs the driver with the library call replaced."""

    def test_arguments_reach_the_library(self):
        """The ref and search directory are passed through."""
        with mock.patch.object(
            griffe_check, "breaking_lines", return_value=[]
        ) as lines:
            code = griffe_check.main(
                ["check", "pkg", "--against", "v1.2.0", "--search", "src"]
            )
        self.assertEqual(code, 0)
        lines.assert_called_once_with("pkg", "v1.2.0", ["src"], None)

    def test_old_search_replaces_the_ref(self):
        """An extracted old wheel can stand in for a git ref."""
        with mock.patch.object(
            griffe_check, "breaking_lines", return_value=[]
        ) as lines:
            code = griffe_check.main(
                ["check", "pkg", "--old-search", "old", "--search", "new"]
            )
        self.assertEqual(code, 0)
        lines.assert_called_once_with("pkg", None, ["new"], ["old"])

    def test_breaking_change_exits_one(self):
        """Any breaking line fails the check."""
        with mock.patch.object(
            griffe_check, "breaking_lines", return_value=["a.py:1: a.f: x"]
        ):
            code = griffe_check.main(
                ["check", "pkg", "--against", "v1", "--search", "src"]
            )
        self.assertEqual(code, 1)


if __name__ == "__main__":
    unittest.main()
