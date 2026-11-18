"""Fetches the terms of every licence in core/license.bzl and checks each against its pinned sha256.

Run by the scheduled terms-check workflow, never in presubmit: vendor pages change on
their own schedule, and a change is a reason to ask users to re-accept, not a red PR.

The hash covers the terms text, not the raw page: for HTML the script, style and
markup are dropped and whitespace is collapsed, because pages such as
developer.android.com embed a fresh nonce and shuffled feature flags in every
response. Other content types (PDF, RTF) are hashed as served.

Usage: python3 -I terms_check.py <path to core/license.bzl>
Prints one line per licence and exits 1 when any hash differs or a fetch fails.
"""

import hashlib
import html
import re
import sys
import urllib.request

_ENTRY = re.compile(r'(\w+): struct\(\s*url = "([^"]+)",\s*terms_sha256 = "([0-9a-f]{64})",?\s*\)')
_ID = re.compile(r'^(\w+) = "([a-z0-9-]+)"$', re.MULTILINE)
_DROP = re.compile(r"<(script|style|noscript)\b.*?</\1\s*>", re.DOTALL | re.IGNORECASE)
_TAG = re.compile(r"<[^>]*>")
_SPACE = re.compile(r"\s+")


def terms_text(body, content_type):
    """Returns the bytes the pinned hash covers."""
    if "html" not in content_type.lower():
        return body
    text = body.decode("utf-8", errors="replace")
    text = _TAG.sub(" ", _DROP.sub(" ", text))
    return _SPACE.sub(" ", html.unescape(text)).strip().encode("utf-8")


def terms_sha256(url):
    """Fetches url and returns the sha256 of its terms text."""
    request = urllib.request.Request(url, headers={"User-Agent": "rules_dx-terms-check"})
    with urllib.request.urlopen(request, timeout=60) as response:
        body = response.read()
        content_type = response.headers.get("Content-Type", "")
    return hashlib.sha256(terms_text(body, content_type)).hexdigest()


def pins(source):
    """Returns (id, url, sha256) for each licence entry in core/license.bzl."""
    ids = dict(_ID.findall(source))
    return [(ids.get(const, const), url, sha) for const, url, sha in _ENTRY.findall(source)]


def main(argv):
    if len(argv) != 2:
        print("usage: terms_check.py <core/license.bzl>", file=sys.stderr)
        return 2
    with open(argv[1], encoding="utf-8") as source:
        entries = pins(source.read())
    if not entries:
        print("terms_check: no licence entries found in " + argv[1], file=sys.stderr)
        return 2
    changed = False
    for name, url, pinned in entries:
        try:
            actual = terms_sha256(url)
        except OSError as error:
            print("{}: fetch failed: {}: {}".format(name, url, error))
            changed = True
            continue
        if actual == pinned:
            print("{}: unchanged".format(name))
        else:
            print("{}: terms changed: {} (pinned {}, now {})".format(name, url, pinned, actual))
            changed = True
    return 1 if changed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
