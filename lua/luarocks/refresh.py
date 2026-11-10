"""Moves rocks in luarocks.lock and rewrites luarocks.digests.json from luarocks.org.

Run through the luarocks_lock target: bazel run //path:lock -- [rock ...]
Each named rock moves to its newest release. Every locked rock is then resolved from
its rockspec, evaluated by the hermetic Lua, and its source is downloaded for the sha256.
It needs network access to luarocks.org and the source hosts.
"""

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
import urllib.request

BASE = "https://luarocks.org"
MANIFEST = "manifest-5.4.json"

_ENTRY = r'^(\s*(?:\["%s"\]|%s)\s*=\s*)"[^"]*"'
_PIN = re.compile(r'^\s*(?:\["([^"]+)"\]|([A-Za-z0-9_]+))\s*=\s*"([^"]*)"\s*,?\s*$')
_DELTAS = {"alpha": -1000000, "beta": -100000, "pre": -10000, "rc": -1000}


class RefreshError(Exception):
    pass


def fetch_bytes(url):
    with urllib.request.urlopen(url, timeout=120) as response:
        return response.read()


def parse_pins(text):
    """Returns {rock: version} from luarocks.lock, without the lua entry."""
    pins = {}
    for line in text.splitlines():
        match = _PIN.match(line)
        if match:
            pins[match.group(1) or match.group(2)] = match.group(3)
    pins.pop("lua", None)
    return pins


def set_pin(text, rock, version):
    """Returns the lock text with one rock pinned to version."""
    pattern = re.compile(_ENTRY % (re.escape(rock), re.escape(rock)), re.MULTILINE)
    updated, count = pattern.subn(lambda m: '%s"%s"' % (m.group(1), version), text)
    if count != 1:
        raise RefreshError("%s is not pinned in luarocks.lock" % rock)
    return updated


def _version_key(version):
    core, revision = version, 0
    head, _, tail = version.rpartition("-")
    if head and tail.isdigit():
        core, revision = head, int(tail)
    parts = []
    for token in re.findall(r"[0-9]+|[A-Za-z]+", core):
        if token.isdigit():
            parts.append((0, int(token)))
        else:
            parts.append((_DELTAS.get(token.lower(), -100 if token.lower() in ("dev", "scm", "cvs", "git", "svn") else -50), 0))
    parts += [(0, 0)] * (16 - len(parts))
    return parts, revision


def is_prerelease(version):
    return any(delta < 0 for delta, _ in _version_key(version)[0])


def latest(manifest, rock):
    """Returns the newest release of rock in the parsed manifest."""
    versions = manifest.get("repository", {}).get(rock)
    if not versions:
        raise RefreshError("%s is not in %s" % (rock, MANIFEST))
    releases = [v for v in versions if not is_prerelease(v)]
    if not releases:
        raise RefreshError("%s has no release in %s" % (rock, MANIFEST))
    return max(releases, key=_version_key)


def resolve_rock(rock, version, fetch, run_resolver, download):
    """Returns the sidecar entry of one locked rock."""
    spec = fetch("%s/%s-%s.rockspec" % (BASE, rock, version))
    resolved = run_resolver(spec, "%s-%s.rockspec" % (rock, version))
    if resolved["package"] != rock or resolved["version"] != version:
        raise RefreshError(
            "%s %s: the rockspec describes %s %s" % (rock, version, resolved["package"], resolved["version"])
        )
    source = resolved["source"]
    return {
        "deps": resolved["deps"],
        "modules": resolved["modules"],
        "source": {
            "sha256": hashlib.sha256(download(source["urls"][0])).hexdigest(),
            "strip_prefix": source["strip_prefix"],
            "urls": source["urls"],
        },
        "version": version,
    }


def refresh(lock_text, names, fetch, run_resolver, download):
    """Returns (new lock text, new sidecar text)."""
    pins = parse_pins(lock_text)
    for name in names:
        if name not in pins:
            raise RefreshError("%s is not pinned in luarocks.lock" % name)
    if names:
        manifest = json.loads(fetch("%s/%s" % (BASE, MANIFEST)))
        for name in names:
            pins[name] = latest(manifest, name)
            lock_text = set_pin(lock_text, name, pins[name])
    packages = {
        rock: resolve_rock(rock, pins[rock], fetch, run_resolver, download) for rock in sorted(pins)
    }
    return lock_text, json.dumps({"packages": packages}, indent=2, sort_keys=True) + "\n"


def resolver_for(binary):
    def run(spec, label):
        with tempfile.TemporaryDirectory() as scratch:
            path = os.path.join(scratch, label)
            with open(path, "wb") as handle:
                handle.write(spec)
            done = subprocess.run([binary, path], capture_output=True, text=True, check=False)
        if done.returncode != 0:
            raise RefreshError(done.stderr.strip() or "%s: the resolver failed" % label)
        return json.loads(done.stdout)

    return run


def main(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", required=True, help="luarocks.lock path relative to the workspace")
    parser.add_argument("--digests", required=True, help="luarocks.digests.json path relative to the workspace")
    parser.add_argument("--resolver", required=True, help="the resolve binary")
    parser.add_argument("rock", nargs="*", help="rocks to move to their newest release")
    args = parser.parse_args(argv)
    resolver = os.path.abspath(args.resolver)
    root = os.environ.get("BUILD_WORKSPACE_DIRECTORY", ".")
    lock = os.path.join(root, args.lock)
    digests = os.path.join(root, args.digests)
    with open(lock, encoding="utf-8") as handle:
        text = handle.read()
    try:
        lock_text, sidecar = refresh(text, args.rock, fetch_bytes, resolver_for(resolver), fetch_bytes)
    except RefreshError as error:
        sys.exit(str(error))
    with open(lock, "w", encoding="utf-8") as handle:
        handle.write(lock_text)
    with open(digests, "w", encoding="utf-8") as handle:
        handle.write(sidecar)
    print("wrote " + lock + " and " + digests)


if __name__ == "__main__":
    main(sys.argv[1:])
