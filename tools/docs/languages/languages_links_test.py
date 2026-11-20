import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import languages  # noqa: E402

BLOB = languages.REPO_BLOB


class RebaseLinksTest(unittest.TestCase):
    def test_consumer_directory_becomes_blob_url(self):
        self.assertEqual(
            languages.rebase_links("[`tests/consumer`](tests/consumer) is minimal.", "flutter"),
            "[`tests/consumer`](%sflutter/tests/consumer) is minimal." % BLOB,
        )

    def test_fixture_file_becomes_blob_url(self):
        self.assertEqual(
            languages.rebase_links("[bench](tests/fixtures/bench/BUILD.bazel)", "haskell"),
            "[bench](%shaskell/tests/fixtures/bench/BUILD.bazel)" % BLOB,
        )

    def test_markdown_outside_docs_stays_relative(self):
        self.assertEqual(
            languages.rebase_links("[x](tests/fixtures/x/README.md)", "ocaml"),
            "[x](../../ocaml/tests/fixtures/x/README.md)",
        )

    def test_root_readme_anchor_stays_relative(self):
        self.assertEqual(
            languages.rebase_links("[q](../README.md#quickstart)", "go"),
            "[q](../../README.md#quickstart)",
        )

    def test_docs_link_stays_relative(self):
        self.assertEqual(
            languages.rebase_links("[g](../docs/guides/x.md)", "go"),
            "[g](../guides/x.md)",
        )

    def test_absolute_and_fenced_links_unchanged(self):
        text = "[a](https://example.com/x)\n```\n[b](tests/consumer)\n```"
        self.assertEqual(languages.rebase_links(text, "flutter"), text)


if __name__ == "__main__":
    unittest.main()
