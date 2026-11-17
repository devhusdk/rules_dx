#!/usr/bin/env python3
"""Generate docs/languages/<lang>.md from <lang>/lang.json, <lang>/defs.bzl and <lang>/README.md.

A language gets a page once <lang>/lang.json and <lang>/defs.bzl both exist.
Run with --write to regenerate, or without to check.
"""

import json
import os
import posixpath
import re
import sys
import tempfile

OUT = "docs/languages"
UPDATE = "bazel run //tools/docs/languages:languages_bin -- --write"
PLATFORM_NAMES = {
    "linux_arm64": "Linux arm64",
    "linux_x86_64": "Linux x86_64",
    "macos_arm64": "macOS arm64",
    "windows_arm64": "Windows arm64",
    "windows_x86_64": "Windows x86_64",
}
LOAD = re.compile(r"^load\(\s*(\"[^\"]+\")\s*,([^\n]*)\)\s*$|^load\(\s*(\"[^\"]+\")\s*,(.*?)\n\)", re.M | re.S)
LOAD_ITEM = re.compile(r"(?:(\w+)\s*=\s*)?\"(\w+)\"")
ALIAS = re.compile(r"^(\w+)\s*=\s*(\w+)\s*$", re.M)
LINK = re.compile(r"\]\(([^)\s#?]+)")
STRING = re.compile(r'"""(.*?)"""|"((?:[^"\\\n]|\\.)*)"', re.S)


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
    for variable in ("DX_SOURCES_JSON", "DX_FILES_JSON", "DX_CONSUMER_JSON"):
        with open(runfile(os.environ[variable]), encoding="utf-8") as handle:
            files = json.load(handle)
        for rel, text in files.items():
            path = os.path.join(root, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", encoding="utf-8", newline="\n") as out:
                out.write(text)
    return root


def languages(root):
    found = []
    for name in sorted(os.listdir(root)):
        base = os.path.join(root, name)
        if os.path.isfile(os.path.join(base, "lang.json")) and os.path.isfile(os.path.join(base, "defs.bzl")):
            found.append(name)
    return found


def rlocation(path):
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if manifest and os.path.isfile(manifest):
        with open(manifest, encoding="utf-8") as handle:
            for line in handle:
                key, _, value = line.rstrip("\n").partition(" ")
                if key == path:
                    return value
    for base in [os.environ.get("RUNFILES_DIR"), sys.argv[0] + ".runfiles"]:
        if base and os.path.exists(os.path.join(base, path)):
            return os.path.join(base, path)
    raise SystemExit("languages: cannot find runfile " + path)


def facts(root):
    """Returns the support matrix rows keyed by name."""
    local = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "support_matrix", "facts.json")
    path = local if os.path.isfile(local) else rlocation("_main/tools/docs/support_matrix/facts.json")
    with open(path, encoding="utf-8") as f:
        return {row["name"]: row for row in json.load(f)["rows"]}


def shard_bzl(root, shard):
    pattern = re.compile(r"^(%s\s*=|def %s\()" % (re.escape(shard), re.escape(shard)), re.M)
    base = os.path.join(root, "tools/docs")
    for name in sorted(os.listdir(base)):
        for bzl in ("defs.bzl", "%s.bzl" % name, "site.bzl"):
            path = "tools/docs/%s/%s" % (name, bzl)
            text = read(root, path)
            if text is not None and pattern.search(text):
                return path
    raise SystemExit("languages: no bzl defines %s" % shard)


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
    margin = min((len(line) - len(line.lstrip()) for line in lines[1:] if line.strip()), default=0)
    return "\n".join([lines[0].strip()] + [line[margin:].rstrip() for line in lines[1:]]).strip() if lines else ""


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
    """Returns (kind, signature, doc, attrs) for the definition behind name, or None.

    A load from another repository returns ("reexport", label, upstream name, []).
    """
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
    factory = re.search(r"^%s\s*=\s*named_source_library\(" % re.escape(name), text, re.M)
    if factory:
        end = balanced(text, factory.end() - 1, "(", ")")
        call = text[factory.end() - 1 : end]
        class_id = re.match(r"\(\s*\"([^\"]*)\"", call)
        strings = {}
        for key in ("doc", "srcs_doc"):
            found = re.search(r"\b%s\s*=\s*(\"\"\".*?\"\"\"|\"(?:[^\"\\\n]|\\.)*\")" % key, call, re.S)
            strings[key] = unquote(parts(STRING.match(found.group(1)))) if found else ""
        if class_id:
            libs = "Other %s libraries this package depends on." % class_id.group(1)
            attrs = [("srcs", "label_list", False, strings["srcs_doc"]), ("deps", "label_list", False, libs)]
            return ("rule", "", strings["doc"], attrs)
    alias = re.search(r"^%s\s*=\s*(\w+)\s*$" % re.escape(name), text, re.M)
    if alias and alias.group(1) != name:
        target = alias.group(1)
        if target in imported:
            return imported_describe(root, path, imported[target], depth)
        return describe(root, path, target, depth + 1)
    if name in imported:
        return imported_describe(root, path, imported[name], depth)
    return None


def imported_describe(root, path, load, depth):
    label, remote = load
    if label.startswith("@"):
        return ("reexport", label, remote, [])
    target_path = label_path(label, path)
    if target_path:
        return describe(root, target_path, remote, depth + 1)
    return None


def bullets(items, fmt="`%s`"):
    return "\n".join("- " + fmt % item for item in items) + "\n" if items else "None.\n"


def render(name, info):
    out = []
    if info is None:
        out.append("No documentation.\n")
        return out
    kind, signature, doc, attrs = info
    if kind == "reexport":
        out.append("Re-exported from `%s`.\n" % signature if doc == name else "Re-exported as `%s` from `%s`.\n" % (doc, signature))
        return out
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
    return out


def rebase_links(text, lang):
    """Rewrites the relative links of <lang>/README.md text to resolve from OUT."""

    def rebase(match):
        target = match.group(1)
        if "://" in target or target.startswith(("mailto:", "/")):
            return match.group(0)
        moved = posixpath.relpath(posixpath.normpath(posixpath.join(lang, target)), OUT)
        return "](" + moved

    out = []
    fenced = False
    for line in text.splitlines():
        if line.lstrip().startswith("```"):
            fenced = not fenced
        out.append(line if fenced else LINK.sub(rebase, line))
    return "\n".join(out)


def readme_sections(root, lang):
    """Returns the `##` sections of <lang>/README.md as an ordered {heading: body} map."""
    text = read(root, "%s/README.md" % lang)
    sections = {}
    if text is None:
        return sections
    heading = None
    fenced = False
    for line in text.splitlines():
        if line.startswith("```"):
            fenced = not fenced
        if not fenced and line.startswith("## "):
            heading = line[3:].strip()
            sections[heading] = []
        elif heading is not None:
            sections[heading].append(line)
    return {h: rebase_links("\n".join(b).strip("\n"), lang) for h, b in sections.items()}


def with_readme(out, extra, heading):
    body = extra.pop(heading, "")
    if body:
        out.append(body + "\n")
    return out


def api_docs(root, lang, rows, extra):
    row = rows.get(lang)
    if row is None or row.get("doc_shard") is None:
        return []
    out = ["## API docs\n"]
    shard = row["doc_shard"]
    if not shard:
        out.append("There is no API doc tool for this language yet.\n")
        return with_readme(out, extra, "API docs")
    path = shard_bzl(root, shard)
    out.append("`tools/dx docs` builds the API docs with `%s`; see [docs](../cli/commands/docs.md).\n" % row["doc"])
    out.append("To declare a docs target yourself, load the rule behind it:\n")
    out.append("```python\nload(\"@rules_dx//%s\", \"%s\")\n```\n" % ("%s:%s" % os.path.split(path), shard))
    out.append("### `%s`\n" % shard)
    out.extend(render(shard, describe(root, path, shard)))
    return with_readme(out, extra, "API docs")


def without_markers(text):
    """Drops the `# dx:` ownership comments that dx writes into files it creates."""
    lines = [line for line in text.splitlines() if not line.startswith("# dx:")]
    return re.sub(r"\n{3,}", "\n\n", "\n".join(lines)).strip("\n")


def quick_start(root, lang, title):
    """Returns the setup steps, using the consumer fixture that CI builds as the example."""
    out = ["## Quick start\n"]
    consumer = "%s/tests/consumer" % lang
    out.append(
        "`MODULE.bazel` sits at the root of your repository and lists the Bazel modules,"
        " meaning external dependencies, that your project uses. Add `rules_dx` to it:\n"
    )
    out.append("```python\nbazel_dep(name = \"rules_dx\", version = \"<release>\")\n```\n")
    out.append("Then let dx set up the rest, as in the [Quickstart](../../README.md#quickstart):\n")
    out.append("```sh\nbazel run @rules_dx//:dx -- init --apply\n```\n")
    module = read(root, "%s/consumer.MODULE.bazel" % consumer)
    if module is not None:
        module = module.replace('bazel_dep(name = "rules_dx", version = "0.0.0")', 'bazel_dep(name = "rules_dx", version = "<release>")')
        out.append("For a %s project, `MODULE.bazel` then looks like this:\n" % title)
        out.append("```python\n%s\n```\n" % without_markers(module))
    build = read(root, "%s/BUILD.bazel" % consumer)
    if build is not None:
        out.append(
            "A `BUILD.bazel` file declares targets: the libraries, binaries and tests that Bazel builds."
            " This one is a small %s package from [`%s`](../../%s/BUILD.bazel):\n" % (title, consumer, consumer)
        )
        out.append("```python\n%s\n```\n" % without_markers(build))
    out.append("Build and test everything. `//...` means every target in the repository:\n")
    out.append("```sh\ntools/dx test //...\n```\n")
    return out


def table(rows):
    out = ["| | |", "| --- | --- |"]
    for label, items, fmt in rows:
        out.append("| %s | %s |" % (label, ", ".join(fmt % item for item in items) if items else "none"))
    return "\n".join(out) + "\n"


def page(root, lang, rows):
    meta = json.loads(read(root, "%s/lang.json" % lang))
    defs = "%s/defs.bzl" % lang
    title = meta["title"]
    names = meta["entry_points"]
    out = ["# %s\n" % title, "<!-- Generated by %s from %s/lang.json and %s. Do not edit. -->\n" % (UPDATE, lang, defs)]
    out.append("This page helps you build, test and check %s code with rules_dx.\n" % title)
    out.extend(quick_start(root, lang, title))
    extra = readme_sections(root, lang)
    platforms = extra.pop("Platforms", "")
    api = api_docs(root, lang, rows, extra)
    # The remaining README sections are task guides (dependencies, quality,
    # toolchain), so they come before the reference generated from lang.json and defs.bzl.
    for heading, body in extra.items():
        out.append("## %s\n" % heading)
        if body:
            out.append(body + "\n")
    out.append("## Rules\n")
    if names:
        shown = ", ".join('"%s"' % n for n in names[:3])
        out.append("`load` brings rules into a `BUILD.bazel` file. Load the ones you use from `@rules_dx//%s:defs.bzl`:\n" % lang)
        out.append("```python\nload(\"@rules_dx//%s:defs.bzl\", %s)\n```\n" % (lang, shown))
    else:
        out.append("None.\n")
    for name in names:
        out.append("### `%s`\n" % name)
        out.extend(render(name, describe(root, defs, name)))
    out.append("## Tools and package files\n")
    out.append("dx runs these tools and reads these package files for %s:\n" % title)
    out.append(
        table(
            [
                ("Tools", meta["tools"], "`%s`"),
                ("Ecosystems", meta["ecosystems"], "%s"),
                ("Manifests", meta["manifests"], "`%s`"),
                ("Lockfiles", meta["lockfiles"], "`%s`"),
            ]
        )
    )
    out.append("## Platforms\n")
    out.append("%s builds and tests on:\n" % title)
    out.append(bullets([PLATFORM_NAMES.get(p, p) for p in meta["platforms"]], "%s"))
    if platforms:
        out.append(platforms + "\n")
    out.extend(api)
    return "\n".join(out).rstrip("\n") + "\n"


def pages(root):
    rows = facts(root)
    return {"%s/%s.md" % (OUT, lang): page(root, lang, rows) for lang in languages(root)}


def find_root():
    if "BUILD_WORKSPACE_DIRECTORY" in os.environ:
        return os.environ["BUILD_WORKSPACE_DIRECTORY"]
    if "DX_SOURCES_JSON" in os.environ:
        return snapshot_root()
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
