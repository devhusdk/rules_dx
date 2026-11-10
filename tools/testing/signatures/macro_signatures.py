#!/usr/bin/env python3
"""Check that binary, test, bench and fuzz macros follow one signature rule."""

import json
import os
import re
import sys
import tempfile

MACRO = re.compile(r"^def ([a-z]\w*_(binary|test|bench|fuzz_test))\(", re.M)
BANNED = ("entry_point",)
REQUIRED_SRCS = {
    "astro_test",
    "cuda_binary",
    "cuda_test",
    "mdx_test",
    "rego_test",
    "svelte_test",
    "typescript_playwright_test",
    "vue_test",
}


def runfile(key):
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if manifest and os.path.isfile(manifest):
        with open(manifest, encoding="utf-8") as handle:
            for line in handle:
                name, _, value = line.rstrip("\n").partition(" ")
                if name == key:
                    return value
    return os.path.join(os.environ["TEST_SRCDIR"], key)


def snapshot_root():
    """Writes the declared source snapshots into a scratch tree and returns its root."""
    root = tempfile.mkdtemp(dir=os.environ.get("TEST_TMPDIR"))
    for variable in ("DX_SOURCES_JSON", "DX_FILES_JSON"):
        with open(runfile(os.environ[variable]), encoding="utf-8") as handle:
            files = json.load(handle)
        for rel, text in files.items():
            path = os.path.join(root, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8", newline="\n") as out:
                out.write(text)
    return root


def parameters(text, start):
    """Returns the top-level parameters of the signature opening at start."""
    found, depth, quote, current = [], 1, None, ""
    for char in text[start:]:
        if quote:
            quote = None if char == quote else quote
        elif char in "\"'":
            quote = char
        elif char in "([{":
            depth += 1
        elif char in ")]}":
            depth -= 1
            if depth == 0:
                break
        elif char == "," and depth == 1:
            found.append(current)
            current = ""
            continue
        current += char
    found.append(current)
    pairs = []
    for raw in found:
        name, _, default = raw.strip().partition("=")
        if name.strip():
            pairs.append((name.strip(), default.strip() if default else None))
    return pairs


def macros(root):
    found = {}
    for lang in sorted(os.listdir(root)):
        path = os.path.join(root, lang, "private", "defs.bzl")
        if not os.path.isfile(
            os.path.join(root, lang, "lang.json")
        ) or not os.path.isfile(path):
            continue
        with open(path, encoding="utf-8") as f:
            text = f.read()
        for match in MACRO.finditer(text):
            kind = (
                "fuzz_test" if match.group(1).endswith("_fuzz_test") else match.group(2)
            )
            found[match.group(1)] = (kind, parameters(text, match.end()))
    return found


def check(macro, kind, params):
    """Returns the rule breaks of one macro signature."""
    names = [name for name, _ in params]
    defaults = dict(params)
    bad = []
    if names[:1] != ["name"]:
        bad.append("name comes first")
    if names[-1:] != ["**kwargs"]:
        bad.append("**kwargs comes last")
    if len(names) > 2 and names[1] != "*":
        bad.append("every parameter after name is keyword-only")
    bad += [
        "%s is not a parameter; use main" % name for name in BANNED if name in names
    ]
    if "srcs" in names:
        if names.index("srcs") != 2:
            bad.append("srcs is the first keyword parameter")
        required = defaults["srcs"] is None
        if not required and defaults["srcs"] != "None":
            bad.append("srcs defaults to None")
        if kind in ("binary", "test") and required != (macro in REQUIRED_SRCS):
            bad.append(
                "srcs defaults to None unless the macro is in REQUIRED_SRCS, and only then"
            )
    elif macro in REQUIRED_SRCS:
        bad.append("REQUIRED_SRCS lists a macro without srcs")
    if "main" in names:
        if names.index("main") != (3 if "srcs" in names else 2):
            bad.append("main follows srcs")
        if defaults["main"] not in (None, "None"):
            bad.append("main defaults to None")
    if defaults.get("visibility", "None") != "None":
        bad.append("visibility defaults to None")
    return bad


def problems(root):
    found = macros(root)
    if not found:
        return ["no binary, test, bench or fuzz macros under %s" % root]
    out = []
    for macro in sorted(REQUIRED_SRCS - set(found)):
        out.append("%s is listed as an exception but does not exist" % macro)
    for macro, (kind, params) in sorted(found.items()):
        out += ["%s: %s" % (macro, bad) for bad in check(macro, kind, params)]
    return out


def main():
    found = problems(snapshot_root())
    for line in found:
        print(line, file=sys.stderr)
    if found:
        sys.exit("macro signatures drift from docs/design/layout.md")
    print("Every binary, test, bench and fuzz macro follows the signature rule")


if __name__ == "__main__":
    main()
