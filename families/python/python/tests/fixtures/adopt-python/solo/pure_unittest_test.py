"""Stdlib unittest coverage for the manifest-free module."""

import unittest

import pure


class CircleAreaTest(unittest.TestCase):
    def test_zero(self) -> None:
        self.assertEqual(pure.circle_area(0.0), 0.0)

    def test_scales_with_square(self) -> None:
        self.assertAlmostEqual(pure.circle_area(2.0), 4.0 * pure.circle_area(1.0))


if __name__ == "__main__":
    unittest.main()
