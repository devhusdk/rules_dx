"""Pure build.zig.zon logic: parsing, digests and spoke BUILD files.

Nothing here runs Zig. Bazel only reads the manifest and its digest sidecar.
"""

def spoke_name(hub, package):
    """Returns the repository name of one package: <hub>__<name>."""
    return hub + "__" + package.replace(".", "_").replace("-", "_")

def _next_quoted(quoted, escaped, ch):
    """Returns (quoted, escaped) after one character of Zig source."""
    if escaped:
        return quoted, False
    if quoted and ch == "\\":
        return quoted, True
    if ch == "\"":
        return not quoted, False
    return quoted, False

def strip_comments(text):
    """Removes `//` comments that are outside string literals."""
    lines = []
    for line in text.split("\n"):
        quoted = False
        escaped = False
        prev = ""
        cut = len(line)
        for i in range(len(line)):
            ch = line[i]
            if not quoted and ch == "/" and prev == "/":
                cut = i - 1
                break
            quoted, escaped = _next_quoted(quoted, escaped, ch)
            prev = "" if quoted else ch
        lines.append(line[:cut])
    return "\n".join(lines)

def _matching_brace(text, start):
    """Returns the index of the `}` closing the brace opened just before `start`."""
    depth = 1
    quoted = False
    escaped = False
    for i in range(start, len(text)):
        ch = text[i]
        if not quoted and ch == "{":
            depth += 1
        elif not quoted and ch == "}":
            depth -= 1
            if depth == 0:
                return i
        quoted, escaped = _next_quoted(quoted, escaped, ch)
    return len(text)

def _string_field(body, field):
    """Returns the string literal assigned to `.field` in a struct body, or ""."""
    key = "." + field
    at = 0
    for _ in range(len(body)):
        found = body.find(key, at)
        if found < 0:
            return ""
        at = found + len(key)
        rest = body[at:].lstrip()
        if not rest.startswith("="):
            continue
        rest = rest[1:].lstrip()
        if not rest.startswith("\""):
            return ""
        end = rest.find("\"", 1)
        return rest[1:end] if end > 0 else ""
    return ""

def _name(raw):
    raw = raw.strip()
    if raw.startswith("@\""):
        return raw[2:raw.find("\"", 2)]
    return raw

def parse_dependencies(text):
    """Parses the `.dependencies` of a build.zig.zon; returns (deps dict, errors).

    Each dep has url and hash. Path dependencies carry no hash and are skipped.
    """
    text = strip_comments(text)
    marker = text.find(".dependencies")
    if marker < 0:
        return {}, []
    open_at = text.find(".{", marker)
    if open_at < 0:
        return {}, []
    block_start = open_at + 2
    block = text[block_start:_matching_brace(text, block_start)]
    deps = {}
    errors = []
    at = 0
    for _ in range(len(block)):
        entry = block.find("= .{", at)
        if entry < 0:
            break
        head = block[at:entry].strip().strip(",").strip()
        name = _name(head.lstrip("."))
        body_start = entry + 4
        body_end = _matching_brace(block, body_start)
        body = block[body_start:body_end]
        at = body_end + 1
        if ".path" in body and ".url" not in body:
            continue
        url = _string_field(body, "url")
        pkg_hash = _string_field(body, "hash")
        if not url:
            errors.append("%s has no .url" % name)
            continue
        if not (url.startswith("https://") or url.startswith("http://")):
            errors.append("%s uses %s; only http(s) archive URLs are supported" % (name, url))
            continue
        if not pkg_hash:
            errors.append("%s has no .hash" % name)
            continue
        deps[name] = struct(url = url, hash = pkg_hash)
    return deps, errors

def archive_type(url):
    """Returns the archive type Bazel extracts a URL as, or "" when unsupported."""
    path = url.split("?")[0].split("#")[0]
    for suffix, kind in [(".tar.gz", "tar.gz"), (".tgz", "tar.gz"), (".tar.xz", "tar.xz"), (".tar.zst", "tar.zst"), (".zip", "zip")]:
        if path.endswith(suffix):
            return kind
    return ""

def parse_digests(text):
    """Parses zig.digests.json; returns (packages dict, errors)."""
    if not text.strip():
        return {}, []
    data = json.decode(text)
    if type(data) != "dict" or type(data.get("packages")) != "dict":
        return {}, ["zig.digests.json must be {\"packages\": {...}}"]
    return data["packages"], []

def pin_problem(pkg_hash, pin):
    """Returns why a package has no usable digest ("" when it is pinned)."""
    if not pin:
        return "has no entry in zig.digests.json"
    if pin.get("hash", "") != pkg_hash:
        return "is pinned to hash %r but build.zig.zon has %r; re-pin it" % (pin.get("hash", ""), pkg_hash)
    if len(pin.get("sha256", "")) != 64:
        return "has no 64 hex character sha256; re-pin it"
    return ""

def pin_entry_json(name, pkg_hash, sha256):
    """Returns the sidecar entry to commit for one package."""
    return json.encode_indent({name: {"hash": pkg_hash, "sha256": sha256}}, indent = "  ")

def analyze(hub, zon_text, digests_text):
    """Validates one manifest and its sidecar.

    Returns a struct with `errors` (empty when usable) and `packages`, a dict keyed by
    package name with: name, spoke, url, hash, sha256 and unpinned (problem text or "").
    """
    deps, errors = parse_dependencies(zon_text)
    pins, digest_errors = parse_digests(digests_text)
    errors += digest_errors
    packages = {}
    for name, dep in deps.items():
        if not archive_type(dep.url):
            errors.append("%s uses %s; supported archives are .tar.gz, .tgz, .tar.xz, .tar.zst and .zip" % (name, dep.url))
            continue
        problem = pin_problem(dep.hash, pins.get(name))
        packages[name] = struct(
            name = name,
            spoke = spoke_name(hub, name),
            url = dep.url,
            hash = dep.hash,
            sha256 = "" if problem else pins[name]["sha256"],
            unpinned = problem,
        )
    owners = {}
    for name in sorted(packages.keys()):
        spoke = packages[name].spoke
        if spoke in owners:
            errors.append("%s and %s both map to the repository name %s; rename one" % (owners[spoke], name, spoke))
        owners[spoke] = name
    for name in sorted(pins.keys()):
        if name not in deps:
            errors.append("zig.digests.json pins %s, which is not in build.zig.zon; remove it" % name)
    return struct(errors = errors, packages = packages)

def main_candidates(name):
    """Returns the module root paths tried for a package, best first."""
    return ["src/%s.zig" % name, "src/root.zig", "%s.zig" % name, "src/main.zig", "root.zig", "main.zig"]

def main_file(name, existing):
    """Returns the module root of a package from the files it contains, or ""."""
    for candidate in main_candidates(name):
        if candidate in existing:
            return candidate
    return ""

def dependency_names(zon_text, selected):
    """Returns (sorted dependency names, errors) of a package's build.zig.zon.

    `selected` maps each root package name to its hash. A dependency that is absent from it,
    or whose hash differs, is an error.
    """
    deps, _ = parse_dependencies(zon_text)
    names = []
    errors = []
    for name in sorted(deps.keys()):
        if name not in selected:
            errors.append("depends on %s, which the root build.zig.zon does not list; add it there" % name)
        elif selected[name] != deps[name].hash:
            errors.append("depends on %s with hash %r but the root build.zig.zon has %r" % (name, deps[name].hash, selected[name]))
        else:
            names.append(name)
    return names, errors

def spoke_build(name, main, dep_spokes, defs_bzl):
    """Returns the BUILD file of one package repository."""
    return """load("{bzl}", "zig_library")

package(default_visibility = ["//visibility:public"])

zig_library(
    name = "pkg",
    srcs = glob(["**/*.zig"], exclude = ["build.zig", {main}], allow_empty = True),
    import_name = {import_name},
    main = {main},
    deps = {deps},
)
""".format(
        bzl = defs_bzl,
        main = repr(main),
        import_name = repr(name),
        deps = repr(["@%s//:pkg" % spoke for spoke in dep_spokes]),
    )

def hub_package_build(name, spoke):
    """Returns the BUILD file the hub holds for one package; @hub//name is its short label."""
    aliases = ["pkg"] if name == "pkg" else ["pkg", name]
    return "package(default_visibility = [\"//visibility:public\"])\n" + "".join([
        "\nalias(\n    name = \"{alias}\",\n    actual = \"@{spoke}//:pkg\",\n)\n".format(alias = alias, spoke = spoke)
        for alias in aliases
    ])

def hub_root_build(packages):
    """Returns the hub's root BUILD file grouping every package."""
    return """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "all_packages",
    srcs = {srcs},
)
""".format(srcs = repr(["//%s:pkg" % name for name in sorted(packages.keys())]))
