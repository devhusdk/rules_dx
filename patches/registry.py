"""Maintains the patch integrity in patches/modules/*/*/source.json."""

import base64
import hashlib
import json
import os
import sys


def _sources(root):
    modules = os.path.join(root, "modules")
    for module in sorted(os.listdir(modules)):
        for version in sorted(os.listdir(os.path.join(modules, module))):
            path = os.path.join(modules, module, version, "source.json")
            if os.path.isfile(path):
                yield path


def _integrity(path):
    with open(path, "rb") as handle:
        digest = hashlib.sha256(handle.read()).digest()
    return "sha256-" + base64.b64encode(digest).decode("ascii")


def _load(source):
    with open(source, encoding="utf-8") as handle:
        return json.load(handle)


def _problems(source, listed):
    directory = os.path.join(os.path.dirname(source), "patches")
    present = set(os.listdir(directory)) if os.path.isdir(directory) else set()
    problems = []
    for name in listed:
        if name not in present:
            problems.append("%s: patch %s is missing" % (source, name))
    for name in sorted(present - set(listed)):
        problems.append("%s: patch %s is not listed" % (source, name))
    return problems


def expected(source):
    """Returns the source.json content with every patch integrity recomputed."""
    data = _load(source)
    listed = data.get("patches", {})
    problems = _problems(source, listed)
    if problems:
        raise SystemExit("\n".join(problems))
    directory = os.path.join(os.path.dirname(source), "patches")
    data["patches"] = {name: _integrity(os.path.join(directory, name)) for name in listed}
    return data


def check(root):
    """Returns the source.json paths whose patch integrity is stale."""
    stale = []
    for source in _sources(root):
        if expected(source) != _load(source):
            stale.append(source)
    return stale


def update(root):
    """Rewrites the patch integrity of every source.json, keeping the patch order."""
    for source in _sources(root):
        data = expected(source)
        with open(source, "w", encoding="utf-8") as handle:
            json.dump(data, handle, indent=4)
            handle.write("\n")


def main(argv):
    if len(argv) != 2 or argv[1] != "update":
        raise SystemExit("usage: update")
    update(os.path.join(os.environ["BUILD_WORKSPACE_DIRECTORY"], "patches"))


if __name__ == "__main__":
    main(sys.argv)
