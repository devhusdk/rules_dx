"""Covers lock bumping and sidecar rendering from recorded luarocks.org answers."""

import hashlib
import json
import unittest

import refresh

LOCK = 'return {\n   dependencies = {\n      inspect = "3.1.3-0",\n      ["penlight"] = "1.13.1-1",\n   },\n}\n'

MANIFEST = {
    "repository": {
        "inspect": {"3.1.3-0": [], "3.1.4-1": [], "3.2.0-beta1-1": [], "3.1.10-0": []},
        "penlight": {"1.13.1-1": []},
    }
}

RESOLVED = {
    "inspect-3.1.10-0.rockspec": {
        "package": "inspect",
        "version": "3.1.10-0",
        "deps": [],
        "modules": {"inspect": "inspect.lua"},
        "source": {"strip_prefix": "inspect.lua-3.1.10", "urls": ["https://example.test/inspect.tar.gz"]},
    },
    "penlight-1.13.1-1.rockspec": {
        "package": "penlight",
        "version": "1.13.1-1",
        "deps": ["luafilesystem"],
        "modules": {"pl.List": "lua/pl/List.lua"},
        "source": {"strip_prefix": "Penlight-1.13.1", "urls": ["https://example.test/penlight.tar.gz"]},
    },
}


def fetch(url):
    if url.endswith(refresh.MANIFEST):
        return json.dumps(MANIFEST).encode()
    return url.rsplit("/", 1)[1].encode()


def run_resolver(spec, label):
    assert spec.decode() == label
    return RESOLVED[label]


def download(url):
    return url.encode()


class RefreshTest(unittest.TestCase):
    def test_parse_pins_reads_both_key_forms(self):
        self.assertEqual(refresh.parse_pins(LOCK), {"inspect": "3.1.3-0", "penlight": "1.13.1-1"})

    def test_latest_orders_numbers_and_skips_prereleases(self):
        self.assertEqual(refresh.latest(MANIFEST, "inspect"), "3.1.10-0")
        with self.assertRaises(refresh.RefreshError):
            refresh.latest(MANIFEST, "missing")

    def test_prerelease_words_sort_below_releases(self):
        self.assertTrue(refresh.is_prerelease("1.0rc1-1"))
        self.assertFalse(refresh.is_prerelease("1.0-1"))
        self.assertLess(refresh._version_key("1.0rc1-1"), refresh._version_key("1.0-1"))
        self.assertLess(refresh._version_key("1.0-1"), refresh._version_key("1.0-2"))

    def test_round_trip_without_names_keeps_pins(self):
        lock, sidecar = refresh.refresh(LOCK.replace("3.1.3-0", "3.1.10-0"), [], fetch, run_resolver, download)
        self.assertEqual(lock, LOCK.replace("3.1.3-0", "3.1.10-0"))
        packages = json.loads(sidecar)["packages"]
        self.assertEqual(sorted(packages), ["inspect", "penlight"])
        self.assertEqual(
            packages["penlight"]["source"]["sha256"],
            hashlib.sha256(b"https://example.test/penlight.tar.gz").hexdigest(),
        )
        self.assertEqual(sidecar, json.dumps(json.loads(sidecar), indent=2, sort_keys=True) + "\n")

    def test_named_rock_moves_to_latest(self):
        lock, sidecar = refresh.refresh(LOCK, ["inspect"], fetch, run_resolver, download)
        self.assertIn('inspect = "3.1.10-0"', lock)
        self.assertIn('["penlight"] = "1.13.1-1"', lock)
        self.assertEqual(json.loads(sidecar)["packages"]["inspect"]["version"], "3.1.10-0")

    def test_unpinned_name_is_refused(self):
        with self.assertRaises(refresh.RefreshError):
            refresh.refresh(LOCK, ["absent"], fetch, run_resolver, download)

    def test_rockspec_for_another_version_is_refused(self):
        def wrong(spec, label):
            return dict(RESOLVED[label], version="9.9-9")

        with self.assertRaises(refresh.RefreshError):
            refresh.refresh(LOCK.replace("3.1.3-0", "3.1.10-0"), [], fetch, wrong, download)


if __name__ == "__main__":
    unittest.main()
