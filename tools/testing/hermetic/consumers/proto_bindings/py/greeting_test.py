import unittest

from py import greeting


class GreetingTest(unittest.TestCase):
    def test_round_trip(self):
        got = greeting.round_trip("py", 2)
        self.assertEqual((got.name, got.times), ("py", 2))


if __name__ == "__main__":
    unittest.main()
