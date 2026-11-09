"""Stdlib unittest fixture for the generic Python test route."""

import unittest

from hello import greet


class GreetTest(unittest.TestCase):
    def test_greet(self) -> None:
        self.assertEqual(greet("world"), "Hello, world!")


if __name__ == "__main__":
    unittest.main()
