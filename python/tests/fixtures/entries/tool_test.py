"""Generic test whose main comes from the target name."""

import unittest

import tool_util


class ToolTest(unittest.TestCase):
    def test_greeting(self) -> None:
        self.assertEqual(tool_util.greeting(), "hello world")


if __name__ == "__main__":
    unittest.main()
