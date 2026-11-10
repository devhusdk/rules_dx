"""Generic unittest run of the built binary."""

import os
import subprocess
import tempfile
import unittest

BINARY = os.path.abspath(os.environ["LEDGER_BIN"])


class BinaryTest(unittest.TestCase):
    def run_binary(self, *args):
        with tempfile.TemporaryDirectory() as cwd:
            return subprocess.run(
                [BINARY, *args], cwd=cwd, capture_output=True, text=True, timeout=60
            )

    def test_list_prints_every_item(self):
        result = self.run_binary("list")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.splitlines(),
            ["bolt-m6\tBolt M6\t45", "nut-m6\tNut M6\t20", "washer-m6\tWasher M6\t8"],
        )

    def test_unknown_command_fails_with_usage(self):
        result = self.run_binary("frobnicate")
        self.assertEqual(result.returncode, 2)
        self.assertIn("No such command", result.stderr)


if __name__ == "__main__":
    unittest.main()
