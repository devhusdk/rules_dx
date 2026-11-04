"""Builds rpds-py from its sdist offline; linux x86_64 only."""

import os
import pathlib
import unittest
from importlib import metadata


class RustSdistTest(unittest.TestCase):
    def test_hash_trie_map(self):
        from rpds import HashTrieMap

        m = HashTrieMap({"a": 1}).insert("b", 2)
        self.assertEqual(m["b"], 2)
        self.assertEqual(len(m), 2)

    def test_installed_from_source(self):
        wheel = metadata.distribution("rpds-py").read_text("WHEEL")
        tags = [line for line in wheel.splitlines() if line.startswith("Tag:")]
        self.assertTrue(tags)
        for tag in tags:
            self.assertTrue(tag.endswith("linux_x86_64"), tag)
            self.assertNotIn("manylinux", tag)


class CargoConfigTest(unittest.TestCase):
    def test_vendored_and_offline(self):
        text = pathlib.Path(os.environ["TEST_SRCDIR"], os.environ["CARGO_CONFIG"]).read_text()
        self.assertIn("offline = true", text)
        self.assertIn("replace-with", text)


if __name__ == "__main__":
    unittest.main()
