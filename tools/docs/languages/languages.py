#!/usr/bin/env python3
"""Generate docs/languages/<lang>.md from <lang>/lang.json and <lang>/defs.bzl.

A language gets a page once <lang>/lang.json and <lang>/defs.bzl both exist.
Run with --write to regenerate, or without to check.
"""

import json
import os
import re
import sys

OUT = "docs/languages"
UPDATE = "bazel run //tools/docs/languages:languages_bin -- --write"
PLATFORM_NAMES = {
    "linux_arm64": "Linux arm64",
    "linux_x86_64": "Linux x86_64",
    "macos_arm64": "macOS arm64",
    "windows_arm64": "Windows arm64",
    "windows_x86_64": "Windows x86_64",
}
LOAD = re.compile(r"^load\(\s*(\"[^\"]+\")\s*,(.*?)\n\)|^load\(\s*(\"[^\"]+\")\s*,([^\n]*)\)", re.M | re.S)
LOAD_ITEM = re.compile(r"(?:(\w+)\s*=\s*)?\"(\w+)\"")
ALIAS = re.compile(r"^(\w+)\s*=\s*(\w+)\s*$", re.M)
STRING = re.compile(r'"""(.*?)"""|"((?:[^"\\\n]|\\.)*)"', re.S)


def languages(root):
    found = []
    for name in sorted(os.listdir(root)):
        base = os.path.join(root, name)
        if os.path.isfile(os.path.join(base, "lang.json")) and os.path.isfile(os.path.join(base, "defs.bzl")):
            found.append(name)
    return found


def read(root, path):
    try:
        with open(os.path.join(root, path), encoding="utf-8") as f:
            return f.read()
    except OSError:
        return None


def label_path(label, current):
    if label.startswith("//"):
        package, _, target = label[2:].partition(":")
        return (package + "/" + target).lstrip("/")
    if label.startswith(":"):
        return os.path.dirname(current) + "/" + label[1:] if os.path.dirname(current) else label[1:]
    return None


def loads(text):
    table = {}
    for match in LOAD.finditer(text):
        label = (match.group(1) or match.group(3)).strip('"')
        body = match.group(2) if match.group(1) else match.group(4)
        for local, remote in LOAD_ITEM.findall(body):
            table[local or remote] = (label, remote)
    return table


def balanced(text, start, opener, closer):
    depth = 0
    index = start
    while index < len(text):
        char = text[index]
        if char in "\"'":
            quote = '"""' if text.startswith('"""', index) else char
            index = text.find(quote, index + len(quote))
            while index != -1 and text[index - 1] == "\\" and quote != '"""':
                index = text.find(quote, index + 1)
            if index == -1:
                return len(text)
            index += len(quote)
            continue
        if char == "#":
            index = text.find("\n", index)
            if index == -1:
                return len(text)
            continue
        if char == opener:
            depth += 1
        elif char == closer:
            depth -= 1
            if depth == 0:
                return index + 1
        index += 1
    return len(text)


def unquote(raw):
    return re.sub(r"\s+", " ", bytes(raw, "utf-8").decode("unicode_escape").strip()) if raw else ""


def parts(match):
    return match.group(1) if match.group(1) is not None else match.group(2)


def dedent(text):
    lines = text.strip("\n").splitlines()
    margin = min((len(l) - len(l.lstrip()) for l in lines[1:] if l.strip()), default=0)
    return "\n".join([lines[0].strip()] + [l[margin:].rstrip() for l in lines[1:]]).strip() if lines else ""


def attrs_of(call):
    start = call.find("attrs")
    if start == -1:
        return []
    brace = call.find("{", start)
    if brace == -1:
        return []
    block = call[brace : balanced(call, brace, "{", "}")]
    found = []
    for item in re.finditer(r"^\s{1,12}\"(\w+)\":\s*attr\.(\w+)\(", block, re.M):
        end = balanced(block, item.end() - 1, "(", ")")
        body = block[item.end() : end]
        doc = re.search(r"\bdoc\s*=\s*(\"\"\".*?\"\"\"|\"(?:[^\"\\\n]|\\.)*\")", body, re.S)
        text = ""
        if doc:
            text = unquote(parts(STRING.match(doc.group(1))))
        mandatory = re.search(r"mandatory\s*=\s*True", body) is not None
        found.append((item.group(1), item.group(2), mandatory, text))
    return found


def describe(root, path, name, depth=0):
    """Returns (kind, signature, doc, attrs) for the definition behind name, or None."""
    if depth > 6:
        return None
    text = read(root, path)
    if text is None:
        return None
    imported = loads(text)
    func = re.search(r"^def %s\(" % re.escape(name), text, re.M)
    if func:
        end = balanced(text, func.end() - 1, "(", ")")
        signature = re.sub(r"\s+", " ", text[func.end() : end - 1]).strip().rstrip(",")
        signature = re.sub(r"\(\s+", "(", signature)
        after = re.match(r"\s*:\s*\n\s*(\"\"\".*?\"\"\"|\"(?:[^\"\\\n]|\\.)*\")", text[end:], re.S)
        doc = dedent(parts(STRING.match(after.group(1)))) if after else ""
        return ("macro", signature, doc, [])
    assign = re.search(r"^%s\s*=\s*(\w+)\(" % re.escape(name), text, re.M)
    if assign and assign.group(1) in ("rule", "macro", "repository_rule", "module_extension"):
        end = balanced(text, assign.end() - 1, "(", ")")
        call = text[assign.end() - 1 : end]
        doc_match = re.search(r"^\s{4}doc\s*=\s*(\"\"\".*?\"\"\"|\"(?:[^\"\\\n]|\\.)*\")", call, re.S | re.M)
        doc = dedent(parts(STRING.match(doc_match.group(1)))) if doc_match else ""
        return (assign.group(1), "", doc, attrs_of(call))
    alias = re.search(r"^%s\s*=\s*(\w+)\s*$" % re.escape(name), text, re.M)
    if alias and alias.group(1) != name:
        target = alias.group(1)
        if target in imported:
            label, remote = imported[target]
            target_path = label_path(label, path)
            if target_path:
                return describe(root, target_path, remote, depth + 1)
        return describe(root, path, target, depth + 1)
    if name in imported:
        label, remote = imported[name]
        target_path = label_path(label, path)
        if target_path:
            return describe(root, target_path, remote, depth + 1)
    return None


def bullets(items, fmt="`%s`"):
    return "\n".join("- " + fmt % item for item in items) + "\n" if items else "None.\n"


def page(root, lang):
    meta = json.loads(read(root, "%s/lang.json" % lang))
    defs = "%s/defs.bzl" % lang
    names = meta["entry_points"]
    out = ["# %s\n" % meta["title"], "<!-- Generated by %s from %s/lang.json and %s. Do not edit. -->\n" % (UPDATE, lang, defs)]
    out.append("## Install\n")
    out.append("Add `rules_dx` to `MODULE.bazel`:\n")
    out.append("```python\nbazel_dep(name = \"rules_dx\", version = \"<version>\")\n```\n")
    if names:
        shown = ", ".join('"%s"' % n for n in names[:3])
        out.append("Load the rules from `@rules_dx//%s:defs.bzl`:\n" % lang)
        out.append("```python\nload(\"@rules_dx//%s:defs.bzl\", %s)\n```\n" % (lang, shown))
    out.append("## Rules\n")
    if not names:
        out.append("None.\n")
    for name in names:
        info = describe(root, defs, name)
        out.append("### `%s`\n" % name)
        if info is None:
            out.append("No documentation.\n")
            continue
        kind, signature, doc, attrs = info
        if signature:
            out.append("```python\n%s(%s)\n```\n" % (name, signature))
        if doc:
            out.append(doc + "\n")
        if attrs:
            out.append("| Attribute | Type | Required | Description |\n| --- | --- | --- | --- |")
            for attr_name, attr_type, mandatory, text in attrs:
                out.append("| `%s` | `%s` | %s | %s |" % (attr_name, attr_type, "yes" if mandatory else "no", text.replace("|", "\\|")))
            out.append("")
        if not (signature or doc or attrs):
            out.append("No documentation.\n")
    out.append("## Tools\n")
    out.append(bullets(meta["tools"]))
    out.append("## Ecosystems\n")
    out.append(bullets(meta["ecosystems"], "%s"))
    out.append("Manifests:\n")
    out.append(bullets(meta["manifests"]))
    out.append("Lockfiles:\n")
    out.append(bullets(meta["lockfiles"]))
    out.append("## Platforms\n")
    out.append(bullets([PLATFORM_NAMES.get(p, p) for p in meta["platforms"]], "%s"))
    return "\n".join(out).rstrip("\n") + "\n"


def pages(root):
    return {"%s/%s.md" % (OUT, lang): page(root, lang) for lang in languages(root)}


def find_root():
    if "BUILD_WORKSPACE_DIRECTORY" in os.environ:
        return os.environ["BUILD_WORKSPACE_DIRECTORY"]
    if "TEST_SRCDIR" in os.environ:
        marker = "docs/languages/zig.md"
        linked = os.path.join(os.environ["TEST_SRCDIR"], os.environ["TEST_WORKSPACE"], marker)
        return os.path.realpath(linked)[: -len(marker)]
    return os.getcwd()


def main():
    root = find_root()
    want = pages(root)
    existing = set()
    directory = os.path.join(root, OUT)
    if os.path.isdir(directory):
        existing = {"%s/%s" % (OUT, n) for n in os.listdir(directory) if n.endswith(".md")}
    if "--write" in sys.argv:
        os.makedirs(directory, exist_ok=True)
        for path in existing - set(want):
            os.remove(os.path.join(root, path))
        for path, text in want.items():
            with open(os.path.join(root, path), "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
        return 0
    stale = sorted(p for p, t in want.items() if read(root, p) != t) + sorted(existing - set(want))
    if stale:
        print("out of date: %s; run: %s" % (", ".join(stale), UPDATE), file=sys.stderr)
        return 1
    print("Language pages match lang.json and defs.bzl")
    return 0


if __name__ == "__main__":
    sys.exit(main())
