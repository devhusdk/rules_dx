#!/usr/bin/env python3
"""Embed example files into docs pages.

A line `<!-- snippet: PATH -->` is followed by a fenced block holding PATH,
indented to the marker. Run with no arguments to rewrite the pages, or with
`--check` to fail when a page differs.
"""

import os
import re
import sys

PAGES = ["docs/github-ci.md"]
MARKER = re.compile(r"^( *)<!-- snippet: (\S+) -->\n( *)```(\w*)\n.*?^\3```\n", re.M | re.S)


def render(text, root):
    def block(match):
        indent, path, _, lang = match.groups()
        with open(os.path.join(root, path)) as handle:
            body = handle.read()
        lines = "".join(
            indent + line if line.strip() else line for line in body.splitlines(True)
        )
        return "%s<!-- snippet: %s -->\n%s```%s\n%s%s```\n" % (
            indent, path, indent, lang, lines, indent,
        )

    return MARKER.sub(block, text)


def main():
    check = "--check" in sys.argv[1:]
    if check:
        root = os.path.join(os.environ["TEST_SRCDIR"], os.environ["TEST_WORKSPACE"])
    else:
        root = os.environ["BUILD_WORKSPACE_DIRECTORY"]
    stale = []
    for page in PAGES:
        path = os.path.join(root, page)
        with open(path) as handle:
            text = handle.read()
        fresh = render(text, root)
        if fresh == text:
            continue
        stale.append(page)
        if not check:
            with open(path, "w") as handle:
                handle.write(fresh)
    if check and stale:
        sys.exit(
            "%s out of date; run bazel run //tools/docs/snippets:update" % ", ".join(stale)
        )


if __name__ == "__main__":
    main()
