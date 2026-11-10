import hashlib
import unittest

import refresh

LOCK = """[dependencies.stdlib]
version = "0.5.0"
url = "https://example.com/stdlib.tar.gz"
sha256 = "old"

[dependencies.json-fortran]
version = "8.3.0"
url = "https://example.com/json.tar.gz"
sha256 = "old"
"""


class RefreshTest(unittest.TestCase):
    def test_all_packages(self):
        out = refresh.refresh(LOCK, set(), lambda url: url.encode())
        self.assertNotIn('"old"', out)
        self.assertIn(hashlib.sha256(b"https://example.com/json.tar.gz").hexdigest(), out)

    def test_named_package_only(self):
        out = refresh.refresh(LOCK, {"stdlib"}, lambda url: b"x")
        self.assertEqual(out.count('"old"'), 1)

    def test_unknown_package(self):
        with self.assertRaises(refresh.RefreshError):
            refresh.refresh(LOCK, {"nope"}, lambda url: b"")


if __name__ == "__main__":
    unittest.main()
