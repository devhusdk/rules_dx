import json
import os
import tempfile
import unittest

import registry

ROOT = os.path.dirname(os.path.abspath(__file__))


def _write(path, text):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)


def _fixture(root, patches, files):
    base = os.path.join(root, "modules", "foo", "1.0.dx.1")
    _write(os.path.join(base, "source.json"), json.dumps({"url": "u", "patches": patches}))
    for name, text in files.items():
        _write(os.path.join(base, "patches", name), text)
    return os.path.join(base, "source.json")


class RegistryTest(unittest.TestCase):
    def test_checked_in_registry_is_current(self):
        self.assertEqual([], registry.check(ROOT))

    def test_stale_integrity(self):
        with tempfile.TemporaryDirectory() as root:
            source = _fixture(root, {"a.patch": "sha256-stale"}, {"a.patch": "x"})
            self.assertEqual([source], registry.check(root))

    def test_update_fixes_integrity_and_keeps_order(self):
        with tempfile.TemporaryDirectory() as root:
            source = _fixture(root, {"b.patch": "", "a.patch": ""}, {"a.patch": "x", "b.patch": "y"})
            registry.update(root)
            with open(source, encoding="utf-8") as handle:
                data = json.load(handle)
            self.assertEqual(["b.patch", "a.patch"], list(data["patches"]))
            self.assertEqual([], registry.check(root))

    def test_missing_patch(self):
        with tempfile.TemporaryDirectory() as root:
            _fixture(root, {"a.patch": ""}, {})
            with self.assertRaises(SystemExit):
                registry.update(root)

    def test_unlisted_patch(self):
        with tempfile.TemporaryDirectory() as root:
            _fixture(root, {}, {"a.patch": "x"})
            with self.assertRaises(SystemExit):
                registry.update(root)


if __name__ == "__main__":
    unittest.main()
