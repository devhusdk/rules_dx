import os
import tempfile
import unittest

import upstream


class FakeGitHub:
    def __init__(self, responses):
        self._responses = responses

    def get(self, path):
        return self._responses.get(path)


def _write(root, name, text):
    path = os.path.join(root, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8") as handle:
        handle.write(text)


class HeaderTest(unittest.TestCase):
    def test_reads_header_above_diff(self):
        text = "Fixes a thing.\nUpstream: a/b#1, c/d#2\n--- a/x\n+++ b/x\n"
        self.assertEqual(upstream.header(text), "a/b#1, c/d#2")
        self.assertEqual(upstream.refs("a/b#1, c/d#2"), [("a/b", 1), ("c/d", 2)])

    def test_ignores_upstream_line_inside_diff(self):
        self.assertIsNone(upstream.header("--- a/x\n+++ b/x\n+Upstream: a/b#1\n"))

    def test_none_needs_reason(self):
        self.assertEqual(upstream.refs("none (rules_dx only)"), [])
        with self.assertRaises(ValueError):
            upstream.refs("none")
        with self.assertRaises(ValueError):
            upstream.refs("https://github.com/a/b/pull/1")


class ReportTest(unittest.TestCase):
    def test_sorts_patches_by_upstream_state(self):
        api = FakeGitHub({
            "/repos/o/r/issues/1": {"state": "closed", "pull_request": {}},
            "/repos/o/r/pulls/1": {"merged": True, "merge_commit_sha": "abc", "merged_at": "2026-02-01T00:00:00Z"},
            "/repos/o/r/releases?per_page=20": [
                {"tag_name": "v2", "draft": False, "prerelease": False, "published_at": "2026-03-01T00:00:00Z"},
                {"tag_name": "v1", "draft": False, "prerelease": False, "published_at": "2026-01-01T00:00:00Z"},
            ],
            "/repos/o/r/compare/v2...abc": {"status": "behind"},
            "/repos/o/r/issues/2": {"state": "closed", "pull_request": {}},
            "/repos/o/r/pulls/2": {"merged": False},
            "/repos/o/r/issues/2/events?per_page=100": [{"event": "closed", "commit_id": None}],
            "/repos/o/r/issues/3": {"state": "open", "pull_request": {}},
            # Closed unmerged by the commit that landed it, as Copybara does.
            "/repos/o/r/issues/5": {"state": "closed", "pull_request": {}},
            "/repos/o/r/pulls/5": {"merged": False},
            "/repos/o/r/issues/5/events?per_page=100": [
                {"event": "labeled", "commit_id": None, "created_at": "2026-02-01T00:00:00Z"},
                {"event": "closed", "commit_id": "c0de", "created_at": "2026-02-02T00:00:00Z"},
            ],
            "/repos/o/r/compare/v2...c0de": {"status": "behind"},
            # Merged, but only on a tag-only repository whose tags predate it.
            "/repos/t/r/issues/4": {"state": "closed", "pull_request": {}},
            "/repos/t/r/pulls/4": {"merged": True, "merge_commit_sha": "def", "merged_at": "2026-02-01T00:00:00Z"},
            "/repos/t/r/releases?per_page=20": [],
            "/repos/t/r/tags?per_page=20": [{"name": "0.1"}],
            "/repos/t/r/compare/0.1...def": {"status": "ahead"},
        })
        with tempfile.TemporaryDirectory() as tmp:
            root = os.path.join(tmp, "patches")
            _write(root, "m/released.patch", "Upstream: o/r#1\n--- a/x\n")
            _write(root, "m/closed.patch", "Upstream: o/r#2\n--- a/x\n")
            _write(root, "m/landed.patch", "Upstream: o/r#5\n--- a/x\n")
            _write(root, "m/open.patch", "Upstream: o/r#3, t/r#4\n--- a/x\n")
            _write(root, "m/local.patch", "Upstream: none (rules_dx only)\n--- a/x\n")
            _write(root, "m/fixed.patch", "Upstream: fixed upstream in v0.9.0\n--- a/x\n")
            _write(root, "m/missing.patch", "--- a/x\n")
            text = upstream.report(api, root)
        self.assertIn("`patches/m/released.patch`: [o/r#1](https://github.com/o/r/pull/1) is in v2", text)
        self.assertIn("`patches/m/landed.patch`: [o/r#5](https://github.com/o/r/pull/5) is in v2", text)
        self.assertIn("`patches/m/fixed.patch`: fixed in v0.9.0", text)
        self.assertNotIn("Unreadable", text)
        self.assertIn("## Upstream ref closed without a merge", text)
        self.assertIn("`patches/m/closed.patch`", text)
        self.assertIn("## No `Upstream:` header\n\n- `patches/m/missing.patch`", text)
        self.assertNotIn("open.patch", text)
        self.assertNotIn("local.patch", text)

    def test_empty_when_nothing_to_do(self):
        api = FakeGitHub({"/repos/o/r/issues/3": {"state": "open"}})
        with tempfile.TemporaryDirectory() as tmp:
            root = os.path.join(tmp, "patches")
            _write(root, "m/open.patch", "Upstream: o/r#3\n--- a/x\n")
            self.assertEqual(upstream.report(api, root), "")


if __name__ == "__main__":
    unittest.main()
