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

    def test_commit_ref_needs_full_lowercase_sha(self):
        sha = "b12fd3fbfb130bb1b90ba03c790f3a8ce101363f"
        self.assertEqual(
            upstream.refs("a/b@%s, c/d#2" % sha), [("a/b", sha), ("c/d", 2)]
        )
        with self.assertRaises(ValueError):
            upstream.refs("a/b@b12fd3f")
        with self.assertRaises(ValueError):
            upstream.refs("a/b@" + sha.upper())

    def test_carry_headers(self):
        text = "# Upstream: a/b#1\n    #Upstream:  c/d#2 \n# Upstream prebuilt on linux\nUpstream: e/f#3\n"
        self.assertEqual(upstream.carry_headers(text), ["a/b#1", "c/d#2"])
        self.assertTrue(upstream.is_carry_file("BUILD"))
        self.assertTrue(upstream.is_carry_file("BUILD.bazel"))
        self.assertTrue(upstream.is_carry_file("defs.bzl"))
        self.assertFalse(upstream.is_carry_file("notes.txt"))


class ReportTest(unittest.TestCase):
    def test_sorts_patches_by_upstream_state(self):
        api = FakeGitHub(
            {
                "/repos/o/r/issues/1": {"state": "closed", "pull_request": {}},
                "/repos/o/r/pulls/1": {
                    "merged": True,
                    "merge_commit_sha": "abc",
                    "merged_at": "2026-02-01T00:00:00Z",
                },
                "/repos/o/r/releases?per_page=20": [
                    {
                        "tag_name": "v2",
                        "draft": False,
                        "prerelease": False,
                        "published_at": "2026-03-01T00:00:00Z",
                    },
                    {
                        "tag_name": "v1",
                        "draft": False,
                        "prerelease": False,
                        "published_at": "2026-01-01T00:00:00Z",
                    },
                ],
                "/repos/o/r/compare/v2...abc": {"status": "behind"},
                "/repos/o/r/issues/2": {"state": "closed", "pull_request": {}},
                "/repos/o/r/pulls/2": {"merged": False},
                "/repos/o/r/issues/2/events?per_page=100": [
                    {"event": "closed", "commit_id": None}
                ],
                "/repos/o/r/issues/3": {"state": "open", "pull_request": {}},
                # Closed unmerged by the commit that landed it, as Copybara does.
                "/repos/o/r/issues/5": {"state": "closed", "pull_request": {}},
                "/repos/o/r/pulls/5": {"merged": False},
                "/repos/o/r/issues/5/events?per_page=100": [
                    {
                        "event": "labeled",
                        "commit_id": None,
                        "created_at": "2026-02-01T00:00:00Z",
                    },
                    {
                        "event": "closed",
                        "commit_id": "c0de",
                        "created_at": "2026-02-02T00:00:00Z",
                    },
                ],
                "/repos/o/r/compare/v2...c0de": {"status": "behind"},
                # Merged, but only on a tag-only repository whose tags predate it.
                "/repos/t/r/issues/4": {"state": "closed", "pull_request": {}},
                "/repos/t/r/pulls/4": {
                    "merged": True,
                    "merge_commit_sha": "def",
                    "merged_at": "2026-02-01T00:00:00Z",
                },
                "/repos/t/r/releases?per_page=20": [],
                "/repos/t/r/tags?per_page=20": [{"name": "0.1"}],
                "/repos/t/r/compare/0.1...def": {"status": "ahead"},
                # A commit pushed without a pull request, released in v2.
                "/repos/o/r/commits/" + "1" * 40: {
                    "commit": {"committer": {"date": "2026-02-01T00:00:00Z"}}
                },
                "/repos/o/r/compare/v2..." + "1" * 40: {"status": "behind"},
                # The BCR has no releases or tags: a merged pull request is published.
                "/repos/bazelbuild/bazel-central-registry/issues/7": {
                    "state": "closed",
                    "pull_request": {},
                },
                "/repos/bazelbuild/bazel-central-registry/pulls/7": {"merged": True},
                "/repos/bazelbuild/bazel-central-registry/issues/8": {
                    "state": "open",
                    "pull_request": {},
                },
                # A commit no release has yet.
                "/repos/o/r/commits/" + "2" * 40: {
                    "commit": {"committer": {"date": "2026-04-01T00:00:00Z"}}
                },
            }
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = os.path.join(tmp, "patches")
            _write(root, "m/released.patch", "Upstream: o/r#1\n--- a/x\n")
            _write(root, "m/closed.patch", "Upstream: o/r#2\n--- a/x\n")
            _write(root, "m/landed.patch", "Upstream: o/r#5\n--- a/x\n")
            _write(root, "m/open.patch", "Upstream: o/r#3, t/r#4\n--- a/x\n")
            _write(root, "m/local.patch", "Upstream: none (rules_dx only)\n--- a/x\n")
            _write(
                root, "m/fixed.patch", "Upstream: fixed upstream in v0.9.0\n--- a/x\n"
            )
            _write(root, "m/missing.patch", "--- a/x\n")
            _write(
                root,
                "m/bcr.patch",
                "Upstream: bazelbuild/bazel-central-registry#7\n--- a/x\n",
            )
            _write(
                root,
                "m/bcr_open.patch",
                "Upstream: bazelbuild/bazel-central-registry#8\n--- a/x\n",
            )
            _write(root, "m/commit.patch", "Upstream: o/r@%s\n--- a/x\n" % ("1" * 40))
            _write(
                root, "m/unreleased.patch", "Upstream: o/r@%s\n--- a/x\n" % ("2" * 40)
            )
            _write(root, "m/unknown.patch", "Upstream: o/r@%s\n--- a/x\n" % ("3" * 40))
            text = upstream.report(api, root)
        self.assertIn(
            "`patches/m/released.patch`: [o/r#1](https://github.com/o/r/pull/1) is in v2",
            text,
        )
        self.assertIn(
            "`patches/m/landed.patch`: [o/r#5](https://github.com/o/r/pull/5) is in v2",
            text,
        )
        self.assertIn("`patches/m/fixed.patch`: fixed in v0.9.0", text)
        self.assertIn(
            "`patches/m/bcr.patch`: [bazelbuild/bazel-central-registry#7]"
            "(https://github.com/bazelbuild/bazel-central-registry/pull/7) is in the BCR",
            text,
        )
        self.assertNotIn("bcr_open.patch", text)
        self.assertIn(
            "`patches/m/commit.patch`: [o/r@1111111](https://github.com/o/r/commit/%s) is in v2"
            % ("1" * 40),
            text,
        )
        self.assertNotIn("unreleased.patch", text)
        self.assertIn(
            "## Unreadable `Upstream:` header\n\n- `patches/m/unknown.patch`: o/r@%s does not exist"
            % ("3" * 40),
            text,
        )
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

    def test_reports_carries(self):
        api = FakeGitHub(
            {
                "/repos/o/r/issues/1": {"state": "closed", "pull_request": {}},
                "/repos/o/r/pulls/1": {
                    "merged": True,
                    "merge_commit_sha": "abc",
                    "merged_at": "2026-02-01T00:00:00Z",
                },
                "/repos/o/r/releases?per_page=20": [
                    {
                        "tag_name": "v2",
                        "draft": False,
                        "prerelease": False,
                        "published_at": "2026-03-01T00:00:00Z",
                    },
                ],
                "/repos/o/r/compare/v2...abc": {"status": "behind"},
                "/repos/o/r/issues/3": {"state": "open", "pull_request": {}},
            }
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = os.path.join(tmp, "patches")
            os.makedirs(root)
            _write(tmp, "cc/x/BUILD.bazel", "x = 1\n    # Upstream: o/r#1\n")
            _write(tmp, "cc/x/defs.bzl", "# Upstream: o/r#3\n")
            _write(tmp, "cc/y/BUILD.bazel", "# Upstream: see docs\n")
            _write(tmp, "cc/z/BUILD.bazel", "# Upstream prebuilt on linux\n")
            _write(tmp, ".git/BUILD.bazel", "# Upstream: o/r#1\n")
            _write(tmp, "cc/x/notes.txt", "# Upstream: o/r#1\n")
            text = upstream.report(api, root)
        self.assertIn(
            "## Upstream fix released: bump the module and delete the patch or carry\n\n"
            "- `cc/x/BUILD.bazel`: [o/r#1](https://github.com/o/r/pull/1) is in v2\n",
            text,
        )
        self.assertIn(
            "## Unreadable `Upstream:` header\n\n- `cc/y/BUILD.bazel`: not an owner/repo#N ref",
            text,
        )
        self.assertNotIn("defs.bzl", text)
        self.assertNotIn("cc/z", text)
        self.assertNotIn(".git", text)
        self.assertNotIn("notes.txt", text)
        self.assertNotIn("No `Upstream:` header", text)

    def test_reports_closed_consumer_fix_issues(self):
        api = FakeGitHub(
            {
                "/repos/devhusdk/rules_dx/issues/10": {"state": "open"},
                "/repos/devhusdk/rules_dx/issues/11": {"state": "closed"},
            }
        )
        readme = (
            "- It ends with `Consumer fix: #N` when consumers need the fix too.\n"
            "| Module | Version | Patches | Why |\n"
            "| --- | --- | --- | --- |\n"
            "| `m` | 1.0 | `a.patch` | Fixes a. Consumer fix: #10 |\n"
            "| `m` | 1.0 | `b.patch` | Fixes b. Consumer fix: #11 |\n"
            "| `m` | 1.0 | `c.patch` | Root only. Fixes c |\n"
        )
        with tempfile.TemporaryDirectory() as tmp:
            root = os.path.join(tmp, "patches")
            _write(root, "README.md", readme)
            text = upstream.report(api, root)
        self.assertEqual(
            text,
            "## Consumer fix issue closed while the patch is applied: reclassify as Root only or name an open issue\n\n"
            "- `m` `b.patch`: [#11](https://github.com/devhusdk/rules_dx/issues/11) is closed\n",
        )


if __name__ == "__main__":
    unittest.main()
