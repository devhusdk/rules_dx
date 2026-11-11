"""Passing conformance subject."""

import unittest

import greet


class PassTest(unittest.TestCase):
    """Greeting is correct."""

    def test_passing_case(self) -> None:
        """Greets the world."""
        self.assertEqual(greet.greet("world"), "hello world")


if __name__ == "__main__":
    unittest.main()
