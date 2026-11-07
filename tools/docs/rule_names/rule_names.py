#!/usr/bin/env python3
"""Keep the Rule names table in docs/cli/commands/README.md in step with defs.bzl.

Rows with an upstream rule are written by hand. Every other public rule or
macro that a defs.bzl or app.bzl in the repo exports gets a generated row with
an empty upstream column. Run with --write to regenerate, or without to check.
"""

import os
import re
import sys

DOC = "docs/cli/commands/README.md"
HEADER = "| Upstream load | Upstream rule | dx load | dx rule |\n| --- | --- | --- | --- |\n"
EXPORT = re.compile(r"^(?:def ([a-z]\w*)\(|([a-z]\w*) = (?:rule|macro)\()", re.M)


def dx_load(path):
    parts = path.split("/")
    if parts[0] == "families":
        return "@rules_dx_%s//%s:%s" % (parts[1], "/".join(parts[2:-1]), parts[-1])
    return "@rules_dx//%s:%s" % ("/".join(parts[:-1]), parts[-1])


def scanned(root):
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if not d.startswith((".", "bazel-"))]
        for name in files:
            if name in ("defs.bzl", "app.bzl"):
                yield os.path.relpath(os.path.join(dirpath, name), root).replace(os.sep, "/")


def exports(root):
    found = {}
    for path in sorted(scanned(root)):
        with open(os.path.join(root, path), encoding="utf-8") as f:
            names = sorted({a or b for a, b in EXPORT.findall(f.read())})
        if names:
            found[dx_load(path)] = names
    return found


def split_table(text):
    start = text.index(HEADER) + len(HEADER)
    end = text.index("\n\n", start) + 1
    return start, end


def cells(row):
    return [c.strip() for c in row.strip().strip("|").split("|")]


def names(cell):
    return re.findall(r"`([^`]+)`", cell)


def render(text, root):
    start, end = split_table(text)
    curated, covered = [], {}
    for row in text[start:end].splitlines():
        upstream_load, upstream, load, rule = cells(row)
        if not upstream_load:
            continue
        curated.append(row)
        covered.setdefault(load.strip("`"), set()).update(names(rule))
    generated = []
    for load, exported in exports(root).items():
        missing = [n for n in exported if n not in covered.get(load, ())]
        for n in missing:
            generated.append("|  |  | `%s` | `%s` |" % (load, n))
    rows = "\n".join(curated + generated) + "\n"
    return text[:start] + rows + text[end:]


def main():
    root = os.getcwd()
    if "BUILD_WORKSPACE_DIRECTORY" in os.environ:
        root = os.environ["BUILD_WORKSPACE_DIRECTORY"]
    elif "TEST_SRCDIR" in os.environ:
        # The test runs unsandboxed, so the README in runfiles is a link into the checkout.
        linked = os.path.join(os.environ["TEST_SRCDIR"], os.environ["TEST_WORKSPACE"], DOC)
        root = os.path.realpath(linked)[: -len(DOC)]
    doc = os.path.join(root, DOC)
    with open(doc, encoding="utf-8") as f:
        text = f.read()
    want = render(text, root)
    if "--write" in sys.argv:
        with open(doc, "w", encoding="utf-8", newline="\n") as f:
            f.write(want)
        return 0
    if want != text:
        print("%s is out of date with the defs.bzl exports; run: bazel run //tools/docs/rule_names:rule_names_bin -- --write" % DOC, file=sys.stderr)
        return 1
    print("Rule names table covers every public export")
    return 0


if __name__ == "__main__":
    sys.exit(main())
