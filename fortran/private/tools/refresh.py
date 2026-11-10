"""Rewrites the sha256 of each package in fpm.lock from its archive.

Run through the fpm_lock target: bazel run //path:lock -- [package ...]
Each named package (all of them when none is named) has its archive downloaded and hashed.
It needs network access to the archive hosts.
"""

import argparse
import hashlib
import os
import re
import sys
import urllib.request


class RefreshError(Exception):
    pass


def fetch_bytes(url):
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read()


_TABLE = re.compile(r'^\s*\[dependencies\.("?)([^\]"]+)\1\]\s*$')
_KEY = re.compile(r'^(\s*(\w+)\s*=\s*)"([^"]*)"(.*)$')


def refresh(text, names, download):
    """Returns the lock text with the sha256 of each selected package re-pinned."""
    lines = text.split("\n")
    tables = {}
    current = None
    for index, line in enumerate(lines):
        table = _TABLE.match(line)
        if table:
            current = table.group(2)
            tables[current] = {}
            continue
        key = _KEY.match(line)
        if current and key:
            tables[current][key.group(2)] = index
    for name in names:
        if name not in tables:
            raise RefreshError("%s is not locked in fpm.lock" % name)
    for name, keys in tables.items():
        if names and name not in names:
            continue
        if "url" not in keys or "sha256" not in keys:
            raise RefreshError("%s needs a url and a sha256 in fpm.lock" % name)
        url = _KEY.match(lines[keys["url"]]).group(3)
        digest = hashlib.sha256(download(url)).hexdigest()
        match = _KEY.match(lines[keys["sha256"]])
        lines[keys["sha256"]] = '%s"%s"%s' % (match.group(1), digest, match.group(4))
    return "\n".join(lines)


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", required=True, help="fpm.lock path relative to the workspace")
    parser.add_argument("package", nargs="*", help="packages to re-hash")
    args = parser.parse_args(argv)
    lock = os.path.join(os.environ.get("BUILD_WORKSPACE_DIRECTORY", "."), args.lock)
    with open(lock, encoding="utf-8") as handle:
        text = handle.read()
    try:
        text = refresh(text, set(args.package), fetch_bytes)
    except (RefreshError, OSError) as error:
        print("fpm_lock: %s" % error, file=sys.stderr)
        return 1
    with open(lock, "w", encoding="utf-8") as handle:
        handle.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
