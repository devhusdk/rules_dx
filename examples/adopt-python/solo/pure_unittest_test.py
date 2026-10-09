"""Unittest adoption without converting to pytest."""

import unittest

import pure


class CircleTest(unittest.TestCase):
    def test_area(self):
        self.assertEqual(pure.circle_area(0.0), 0.0)
        self.assertEqual(pure.circle_area(1.0), pure.circle_area(1.0))
