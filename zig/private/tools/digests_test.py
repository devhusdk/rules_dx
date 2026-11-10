import hashlib
import json
import unittest

import digests

ZON = """.{
    // .x = .{ .url = "https://no" },
    .name = .demo,
    .dependencies = .{
        .xxhash = .{
            .url = "https://example.com/xxhash.tar.gz",
            .hash = "xxhash-1-AAAA",
        },
        .@"a-b" = .{
            .url = "https://example.com/ab.tar.gz",
            .hash = "ab-1-BBBB",
        },
        .local = .{ .path = "../local" },
    },
}
"""


class DigestsTest(unittest.TestCase):
    def test_parse_keeps_archive_dependencies(self):
        self.assertEqual(
            digests.parse_dependencies(ZON),
            {
                "xxhash": ("https://example.com/xxhash.tar.gz", "xxhash-1-AAAA"),
                "a-b": ("https://example.com/ab.tar.gz", "ab-1-BBBB"),
            },
        )

    def test_refresh_hashes_every_archive(self):
        out = json.loads(digests.refresh(ZON, "", set(), lambda url: url.encode()))
        self.assertEqual(
            out["packages"]["xxhash"],
            {"hash": "xxhash-1-AAAA", "sha256": hashlib.sha256(b"https://example.com/xxhash.tar.gz").hexdigest()},
        )
        self.assertEqual(sorted(out["packages"]), ["a-b", "xxhash"])

    def test_refresh_keeps_unnamed_entries(self):
        old = json.dumps({"packages": {"a-b": {"hash": "ab-1-BBBB", "sha256": "old"}}})
        out = json.loads(digests.refresh(ZON, old, {"xxhash"}, lambda url: b"x"))
        self.assertEqual(out["packages"]["a-b"]["sha256"], "old")
        self.assertEqual(out["packages"]["xxhash"]["sha256"], hashlib.sha256(b"x").hexdigest())

    def test_unnamed_missing_package_is_skipped(self):
        seen = []

        def download(url):
            seen.append(url)
            return b"x"

        out = json.loads(digests.refresh(ZON, "", {"xxhash"}, download))
        self.assertEqual(seen, ["https://example.com/xxhash.tar.gz"])
        self.assertEqual(list(out["packages"]), ["xxhash"])

    def test_empty_hash_is_refused(self):
        zon = '.{ .dependencies = .{ .a = .{ .url = "https://example.com/a.tar.gz" } } }'
        with self.assertRaises(digests.DigestsError):
            digests.refresh(zon, "", set(), lambda url: b"x")

    def test_unknown_package_is_refused(self):
        with self.assertRaises(digests.DigestsError):
            digests.refresh(ZON, "", {"nope"}, lambda url: b"")


if __name__ == "__main__":
    unittest.main()
