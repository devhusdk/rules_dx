"""Failing conformance subject."""

import unittest

import greet


class FailTest(unittest.TestCase):
    """Greeting is wrong on purpose."""

    def test_failing_case(self) -> None:
        """Expects a greeting that is not produced."""
        self.assertEqual(greet.greet("world"), "goodbye world")


if __name__ == "__main__":
    unittest.main()
