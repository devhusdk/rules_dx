"""Checks that every patch under patches/ has a readable Upstream header."""

import json
import os
import unittest

import upstream


def runfile(key):
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if manifest and os.path.isfile(manifest):
        with open(manifest, encoding="utf-8") as handle:
            for line in handle:
                name, _, value = line.rstrip("\n").partition(" ")
                if name == key:
                    return value
    return os.path.join(os.environ["TEST_SRCDIR"], key)


def problem(text):
    """Returns why a patch header is unreadable, or None when it is fine."""
    value = upstream.header(text)
    if value is None:
        return "no Upstream: line above the diff"
    if upstream._FIXED.match(value):
        return None
    try:
        upstream.refs(value)
    except ValueError as error:
        return str(error)
    return None


class HeadersTest(unittest.TestCase):
    def test_problem(self):
        self.assertIsNone(problem("Upstream: a/b#1, c/d#2\n"))
        self.assertIsNone(problem("Upstream: none (rules_dx only)\n"))
        self.assertIsNone(problem("Upstream: fixed upstream in v1.0.0\n"))
        self.assertIsNotNone(problem(""))
        self.assertIsNotNone(problem("Upstream: none\n"))
        self.assertIsNotNone(problem("Upstream: a/b 1\n"))

    def test_every_patch_has_a_header(self):
        with open(runfile(os.environ["DX_PATCH_HEADERS_JSON"]), encoding="utf-8") as handle:
            headers = json.load(handle)
        self.assertGreater(len(headers), 100)
        failures = []
        for path, text in sorted(headers.items()):
            reason = problem(text)
            if reason:
                failures.append("%s: %s" % (path, reason))
        if failures:
            self.fail(
                "These patches need an Upstream: header line above the diff,"
                " as patches/README.md step 1 describes:\n" + "\n".join(failures)
            )


if __name__ == "__main__":
    unittest.main()
