"""Rewrites zig.digests.json from the archives named in build.zig.zon.

Run through the zig_digests target: bazel run //path:digests -- [package ...]
Each named package (all of them when none is named) has its archive downloaded and hashed.
It needs network access to the archive hosts.
"""

import argparse
import hashlib
import json
import os
import re
import sys
import urllib.request


class DigestsError(Exception):
    pass


def fetch_bytes(url):
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read()


def strip_comments(text):
    lines = []
    for line in text.split("\n"):
        quoted = False
        cut = len(line)
        for i, ch in enumerate(line):
            if ch == '"' and (i == 0 or line[i - 1] != "\\"):
                quoted = not quoted
            if not quoted and line.startswith("//", i):
                cut = i
                break
        lines.append(line[:cut])
    return "\n".join(lines)


def _block(text, start):
    """Returns the text between the brace before start and its match."""
    depth = 1
    quoted = False
    for i in range(start, len(text)):
        ch = text[i]
        if ch == '"' and text[i - 1] != "\\":
            quoted = not quoted
        elif not quoted and ch == "{":
            depth += 1
        elif not quoted and ch == "}":
            depth -= 1
            if depth == 0:
                return text[start:i]
    raise DigestsError("build.zig.zon has an unclosed brace")


_ENTRY = re.compile(r'\.(?:@"([^"]+)"|([A-Za-z_][A-Za-z0-9_]*))\s*=\s*\.\{')


def _field(body, field):
    match = re.search(r'\.%s\s*=\s*"([^"]*)"' % field, body)
    return match.group(1) if match else ""


def parse_dependencies(text):
    """Returns {name: (url, hash)} for the dependencies that name an archive URL."""
    text = strip_comments(text)
    marker = re.search(r"\.dependencies\s*=\s*\.\{", text)
    if not marker:
        return {}
    body = _block(text, marker.end())
    found = {}
    at = 0
    while True:
        entry = _ENTRY.search(body, at)
        if not entry:
            return found
        inner = _block(body, entry.end())
        at = entry.end() + len(inner) + 1
        url = _field(inner, "url")
        if url:
            found[entry.group(1) or entry.group(2)] = (url, _field(inner, "hash"))


def refresh(zon_text, old_text, names, download):
    """Returns the new digests text."""
    dependencies = parse_dependencies(zon_text)
    for name in names:
        if name not in dependencies:
            raise DigestsError("%s is not an archive dependency in build.zig.zon" % name)
    old = json.loads(old_text).get("packages", {}) if old_text.strip() else {}
    packages = {}
    for name in sorted(dependencies):
        url, package_hash = dependencies[name]
        if names and name not in names and name in old:
            packages[name] = old[name]
            continue
        packages[name] = {"hash": package_hash, "sha256": hashlib.sha256(download(url)).hexdigest()}
    return json.dumps({"packages": packages}, indent=2, sort_keys=True) + "\n"


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--zon", required=True, help="build.zig.zon path relative to the workspace")
    parser.add_argument("--digests", required=True, help="zig.digests.json path relative to the workspace")
    parser.add_argument("package", nargs="*", help="packages to re-hash")
    args = parser.parse_args(argv)
    root = os.environ.get("BUILD_WORKSPACE_DIRECTORY", ".")
    zon = os.path.join(root, args.zon)
    digests = os.path.join(root, args.digests)
    with open(zon, encoding="utf-8") as handle:
        zon_text = handle.read()
    old_text = ""
    if os.path.exists(digests):
        with open(digests, encoding="utf-8") as handle:
            old_text = handle.read()
    try:
        text = refresh(zon_text, old_text, set(args.package), fetch_bytes)
    except (DigestsError, OSError, ValueError) as error:
        print("zig_digests: %s" % error, file=sys.stderr)
        return 1
    with open(digests, "w", encoding="utf-8") as handle:
        handle.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
