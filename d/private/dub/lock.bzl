"""Pure dub.selections.json logic: parsing, digests, download URLs and spoke BUILD files.

Nothing here runs DUB. DUB wrote the selections file; Bazel only reads it.
"""

_DEFAULT_SOURCE_DIRS = ["source", "src"]

def spoke_name(hub, package):
    """Returns the repository name of one package: <hub>__<name>."""
    return hub + "__" + package.replace(":", "_")

def package_url(name, version):
    """Returns the code.dlang.org archive URL of one package version."""
    return "https://code.dlang.org/packages/%s/%s.zip" % (name, version)

def parse_selections(text):
    """Parses dub.selections.json; returns (versions dict, errors)."""
    data = json.decode(text)
    if data.get("fileVersion") != 1:
        return {}, ["dub.selections.json must have \"fileVersion\": 1, got %r" % data.get("fileVersion")]
    versions = {}
    errors = []
    for name, value in sorted(data.get("versions", {}).items()):
        if type(value) == "string":
            versions[name] = value
        else:
            errors.append("%s is selected by path or repository; only plain versions are supported" % name)
    return versions, errors

def parse_digests(text):
    """Parses dub.digests.json; returns (packages dict, errors)."""
    if not text.strip():
        return {}, []
    data = json.decode(text)
    if type(data) != "dict" or type(data.get("packages")) != "dict":
        return {}, ["dub.digests.json must be {\"packages\": {...}}"]
    return data["packages"], []

def pin_problem(version, pin):
    """Returns why a package has no usable digest ("" when it is pinned)."""
    if not pin:
        return "has no entry in dub.digests.json"
    if pin.get("version", "") != version:
        return "is pinned to version %r but dub.selections.json has %r; re-pin it" % (pin.get("version", ""), version)
    if len(pin.get("sha256", "")) != 64:
        return "has no 64 hex character sha256; re-pin it"
    return ""

def pin_entry_json(name, version, sha256):
    """Returns the sidecar entry to commit for one package."""
    return json.encode_indent({name: {"version": version, "sha256": sha256}}, indent = "  ")

def analyze(hub, selections_text, digests_text):
    """Validates one selections file and its sidecar.

    Returns a struct with `errors` (empty when usable) and `packages`, a dict keyed by
    package name with: name, version, spoke, url, sha256 and unpinned (problem text or "").
    """
    versions, errors = parse_selections(selections_text)
    pins, digest_errors = parse_digests(digests_text)
    errors += digest_errors
    packages = {}
    for name, version in versions.items():
        pin = pins.get(name)
        problem = pin_problem(version, pin)
        packages[name] = struct(
            name = name,
            version = version,
            spoke = spoke_name(hub, name),
            url = package_url(name, version),
            sha256 = "" if problem else pin["sha256"],
            unpinned = problem,
        )
    for name in sorted(pins.keys()):
        if name not in versions:
            errors.append("dub.digests.json pins %s, which is not in dub.selections.json; remove it" % name)
    return struct(errors = errors, packages = packages)

def default_source_dir(existing):
    """Returns the source directory DUB assumes when dub.json names none, from the directories that exist."""
    for path in _DEFAULT_SOURCE_DIRS:
        if path in existing:
            return path
    return ""

def dependency_names(manifest, selected):
    """Returns the sorted selected packages that a dub.json depends on; sub-package names count as their base package."""
    names = []
    for key in manifest.get("dependencies", {}).keys():
        base = key.split(":")[0]
        if base in selected and base not in names:
            names.append(base)
    return sorted(names)

def spoke_build(name, manifest, existing_dirs, dep_spokes, defs_bzl):
    """Returns the BUILD file of one package repository.

    `manifest` is the decoded dub.json, `existing_dirs` the top-level directories of the
    archive and `dep_spokes` the repository names of the package's selected dependencies.
    """
    source_paths = manifest.get("sourcePaths") or [p for p in [default_source_dir(existing_dirs)] if p]
    source_files = manifest.get("sourceFiles") or []
    if not source_paths and not source_files:
        fail("%s has no sourcePaths or sourceFiles in dub.json and no source or src directory" % name)
    import_paths = manifest.get("importPaths") or source_paths
    string_paths = manifest.get("stringImportPaths") or (["views"] if "views" in existing_dirs else [])
    srcs = []
    for path in source_paths:
        srcs += ["%s/**/*.d" % path.rstrip("/"), "%s/**/*.di" % path.rstrip("/")]
    srcs += source_files
    string_srcs = ["%s/**" % path.rstrip("/") for path in string_paths]
    string_attrs = ""
    if string_paths:
        string_attrs = "    string_imports = %s,\n    string_srcs = glob(%s),\n" % (repr([p.rstrip("/") for p in string_paths]), repr(string_srcs))
    return """load("{bzl}", "d_library")

package(default_visibility = ["//visibility:public"])

d_library(
    name = "pkg",
    srcs = glob({srcs}, allow_empty = True),
    deps = {deps},
    imports = {imports},
{string_attrs})
""".format(
        bzl = defs_bzl,
        srcs = repr(srcs),
        deps = repr(["@%s//:pkg" % spoke for spoke in dep_spokes]),
        imports = repr([p.rstrip("/") for p in import_paths]),
        string_attrs = string_attrs,
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
