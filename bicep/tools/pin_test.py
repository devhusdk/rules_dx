"""Covers reference parsing and digest rendering, with no network."""

import hashlib
import unittest

import pin


class PinTest(unittest.TestCase):
    def test_parse_ref(self):
        self.assertEqual(pin.parse_ref("br:mcr.microsoft.com/bicep/a/b:1.0"), ("mcr.microsoft.com", "bicep/a/b", "1.0"))

    def test_parse_ref_rejects_missing_tag(self):
        for ref in ["mcr.microsoft.com/a:1", "br:host/a", "br:host/a:", "br:host:1"]:
            with self.assertRaises(ValueError, msg=ref):
                pin.parse_ref(ref)

    def test_digest_is_sha256_of_manifest(self):
        body = b'{"schemaVersion": 2}'
        self.assertEqual(pin.digest_of(body), "sha256:" + hashlib.sha256(body).hexdigest())


if __name__ == "__main__":
    unittest.main()
