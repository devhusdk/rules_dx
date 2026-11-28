#!/usr/bin/env python3
"""Keep the Rule names table in docs/cli/commands/README.md in step with lang.json.

Rows with an upstream rule are written by hand. Every other entry point listed
in a <lang>/lang.json gets a generated row with an empty upstream column. Run with --write to regenerate, or without to check.
"""

import json
import os
import re
import sys
import tempfile

DOC = "docs/cli/commands/README.md"
HEADER = (
    "| Upstream load | Upstream rule | dx load | dx rule |\n| --- | --- | --- | --- |\n"
)


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


def entry_bzl(root, lang, name):
    """Returns the top-level <lang>/*.bzl basename that defines name, preferring defs.bzl."""
    pattern = re.compile(
        r"^(%s\s*=|def %s\()" % (re.escape(name), re.escape(name)), re.M
    )
    others = sorted(
        n
        for n in os.listdir(os.path.join(root, lang))
        if n.endswith(".bzl") and n != "defs.bzl"
    )
    for bzl in ["defs.bzl"] + others:
        path = os.path.join(root, lang, bzl)
        if os.path.isfile(path):
            with open(path, encoding="utf-8") as f:
                if pattern.search(f.read()):
                    return bzl
    return "defs.bzl"


def exports(root):
    found = {}
    for lang in sorted(os.listdir(root)):
        path = os.path.join(root, lang, "lang.json")
        if lang.startswith((".", "bazel-")) or not os.path.isfile(path):
            continue
        with open(path, encoding="utf-8") as f:
            names = sorted(set(json.load(f).get("entry_points", [])))
        for name in names:
            load = "@rules_dx//%s:%s" % (lang, entry_bzl(root, lang, name))
            found.setdefault(load, []).append(name)
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
    elif "DX_SOURCES_JSON" in os.environ:
        root = snapshot_root()
    doc = os.path.join(root, DOC)
    with open(doc, encoding="utf-8") as f:
        text = f.read()
    want = render(text, root)
    if "--write" in sys.argv:
        with open(doc, "w", encoding="utf-8", newline="\n") as f:
            f.write(want)
        return 0
    if want != text:
        print(
            "%s is out of date with the lang.json entry points; run: bazel run //tools/docs/rule_names:rule_names_bin -- --write"
            % DOC,
            file=sys.stderr,
        )
        return 1
    print("Rule names table covers every lang.json entry point")
    return 0


if __name__ == "__main__":
    sys.exit(main())
