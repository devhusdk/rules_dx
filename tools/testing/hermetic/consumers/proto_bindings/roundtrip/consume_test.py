import os
import unittest

from api.v1 import greeting_pb2


class ConsumeTest(unittest.TestCase):
    def test_parses_what_go_wrote(self):
        path = os.path.join(os.path.dirname(__file__), "greeting.bin")
        with open(path, "rb") as f:
            greeting = greeting_pb2.Greeting.FromString(f.read())
        self.assertEqual(greeting.name, "from go")
        self.assertEqual(greeting.times, 3)


if __name__ == "__main__":
    unittest.main()
