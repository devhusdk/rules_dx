#!/usr/bin/env python3
"""Check that each bench and fuzz macro in a private/defs.bzl has a language row."""

import os
import re
import sys

LANGUAGES = os.path.join("tools", "docs", "support_matrix", "languages.bzl")
FIELDS = {"bench": "bench", "fuzz_test": "fuzz"}
MACRO = re.compile(r"^def ([a-z]\w*)_(bench|fuzz_test)\(", re.M)
ROW = re.compile(r'^    _language\(\n        "(\w+)",\n(.*?)^    \),$', re.M | re.S)
FIELD = re.compile(r'^        (bench|fuzz) = "(\w+)",$', re.M)


def checkout_root():
    # Unsandboxed, so the runfiles copy of languages.bzl links into the checkout.
    linked = os.path.join(
        os.environ["TEST_SRCDIR"], os.environ["TEST_WORKSPACE"], LANGUAGES
    )
    real = os.path.realpath(linked)
    if not real.endswith(LANGUAGES):
        sys.exit("cannot find the checkout from %s" % linked)
    return real[: -len(LANGUAGES)]


def macros(root):
    found = {}
    for dirpath, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if not d.startswith((".", "bazel-"))]
        if os.path.basename(dirpath) != "private" or "defs.bzl" not in files:
            continue
        path = os.path.join(dirpath, "defs.bzl")
        with open(path, encoding="utf-8") as f:
            for lang, kind in MACRO.findall(f.read()):
                rel = os.path.relpath(path, root).replace(os.sep, "/")
                found[(lang, kind)] = rel
    return found


def entries(text):
    found = ROW.findall(text)
    if not found:
        sys.exit("no _language rows in %s" % LANGUAGES)
    rows = {field: {} for field in FIELDS.values()}
    for lang, body in found:
        for field, macro in FIELD.findall(body):
            rows[field][lang] = macro
    return rows


def problems(root):
    with open(os.path.join(root, LANGUAGES), encoding="utf-8") as f:
        text = f.read()
    found = macros(root)
    if not found:
        return ["no bench or fuzz macros found under %s" % root]
    rows = entries(text)
    out = []
    for (lang, kind), path in sorted(found.items()):
        macro = "%s_%s" % (lang, kind)
        if rows[FIELDS[kind]].get(lang) != macro:
            out.append(
                "%s in %s has no %s row in %s" % (macro, path, FIELDS[kind], LANGUAGES)
            )
    return out


def main():
    bad = problems(checkout_root())
    for line in bad:
        print(line, file=sys.stderr)
    if bad:
        return 1
    print("Every bench and fuzz macro has a support matrix row")
    return 0


if __name__ == "__main__":
    sys.exit(main())
