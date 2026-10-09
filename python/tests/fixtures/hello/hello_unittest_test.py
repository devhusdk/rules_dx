"""Proves wrapper unittest discovery without pytest."""

import unittest

from hello import greet


class GreetTest(unittest.TestCase):
    def test_greet(self):
        self.assertEqual(greet("world"), "Hello, world!")
