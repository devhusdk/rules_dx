"""Hermetic OCaml acquisition and toolchain registration."""

load("//core:toolchain.bzl", "toolchain_target")
load(":pins.bzl", "OCAML_CELLS", "OCAML_VERSION")

HUB_NAME = "dx_ocaml_toolchains"

DIST_PREFIX = "dx_ocaml_"

VERSION_FILE = "DX_OCAML_VERSION"

DROPPED_PATHS = [
    ".bazelignore",
    ".bazelrc",
    ".gitignore",
    "MODULE.bazel",
    "README.adoc",
    "docs",
    "stardoc",
    "tools",
]

EMITTERS = {
    "sys": struct(
        exec_compatible_with = [str(Label("//ocaml/toolchain/platform/emitter:sys"))],
        target_compatible_with = [str(Label("//ocaml/toolchain/platform/executor:sys"))],
    ),
    "vm": struct(
        exec_compatible_with = [str(Label("//ocaml/toolchain/platform/emitter:vm"))],
        target_compatible_with = [str(Label("//ocaml/toolchain/platform/executor:vm"))],
    ),
}

_DIST_BUILD = """load("@rules_dx//ocaml/toolchain:ocaml.bzl", "dx_ocaml_dist")
load("@rules_ocaml//build:rules.bzl", "ocaml_import")

package(default_visibility = ["//visibility:public"])

exports_files(glob(["**"]))

dx_ocaml_dist(
    name = "dist",
    emitters = {emitters},
    licenses = {licenses},
    version_file = "{version_file}",
)

ocaml_import(
    name = "unix",
    afiles = select({{
        "@rules_dx//ocaml/toolchain:emitter_sys": [
            "lib/ocaml/libunixnat.a",
            "lib/ocaml/unix/unix.a",
        ],
        "//conditions:default": [],
    }}),
    archive = select({{
        "@rules_dx//ocaml/toolchain:emitter_sys": "lib/ocaml/unix/unix.cmxa",
        "//conditions:default": "lib/ocaml/unix/unix.cma",
    }}),
    sigs = glob(["lib/ocaml/unix/*.cmi", "lib/ocaml/unix/*.cmx"]),
)

ocaml_import(
    name = "str",
    afiles = select({{
        "@rules_dx//ocaml/toolchain:emitter_sys": [
            "lib/ocaml/libcamlstrnat.a",
            "lib/ocaml/str/str.a",
        ],
        "//conditions:default": [],
    }}),
    archive = select({{
        "@rules_dx//ocaml/toolchain:emitter_sys": "lib/ocaml/str/str.cmxa",
        "//conditions:default": "lib/ocaml/str/str.cma",
    }}),
    sigs = glob(["lib/ocaml/str/*.cmi", "lib/ocaml/str/*.cmx"]),
)
"""

_CELL = """config_setting(
    name = "{cell}",
    constraint_values = {constraints},
)
"""

_OCAMLRUN = """alias(
    name = "ocamlrun",
    actual = select({cases}),
    visibility = ["//visibility:public"],
)
"""

_ALIAS = """alias(
    name = "{name}",
    actual = select({cases}),
    visibility = ["//visibility:public"],
)
"""

# Aliases that also resolve on exec platforms without an OCaml cell, to an empty filegroup, so an
# aspect that lists them as implicit dependencies still analyzes there.
EMPTY_DEFAULT_ALIASES = ["stdlib_cmis"]

_NONE = """filegroup(
    name = "none",
    visibility = ["//visibility:public"],
)
"""

# Aliases of the hub that select the target cell's own label: name -> label in the dist repository.
CELL_ALIASES = {
    "stdlib": "//:stdlib",
    "stdlib_cmi": "//:lib/ocaml/stdlib.cmi",
    "stdlib_cmis": "//:stdlib_cmis",
    "str": "//:str",
    "tree": "//:tree",
    "unix": "//:unix",
}

def relative_prefix(path):
    """Returns the path from one file's directory back to the repository root."""
    depth = len(path.split("/")) - 1
    return "/".join([".."] * depth) if depth else "."

def placeholder_files(paths, mode):
    """Returns the files of a conda paths.json that carry a placeholder of one mode."""
    return sorted([
        entry["_path"]
        for entry in paths
        if entry.get("prefix_placeholder") and entry.get("file_mode", "text") == mode
    ])

def conda_stem(url):
    """Returns the package file name without its .conda suffix."""
    name = url.split("/")[-1]
    if not name.endswith(".conda"):
        fail("OCaml package " + url + " is not a .conda archive")
    return name[:-len(".conda")]

def _install_package(rctx, package):
    stem = conda_stem(package.archive.urls[0])
    rctx.download(url = package.archive.urls, output = "_conda/" + stem + ".conda", sha256 = package.archive.sha256)
    rctx.extract("_conda/" + stem + ".conda", output = "_conda/" + stem, type = "zip")
    rctx.extract("_conda/" + stem + "/pkg-" + stem + ".tar.zst", output = ".")
    rctx.extract("_conda/" + stem + "/info-" + stem + ".tar.zst", output = "_conda/" + stem + "/info")
    info = "_conda/" + stem + "/info/info/"
    paths = json.decode(rctx.read(info + "paths.json"))["paths"]
    binary = placeholder_files(paths, "binary")
    if binary != sorted(package.binary_placeholders):
        fail("OCaml package " + package.name + " has unreviewed binary placeholders: " + repr(binary))
    for entry in paths:
        placeholder = entry.get("prefix_placeholder")
        if not placeholder or entry.get("file_mode", "text") != "text":
            continue
        path = entry["_path"]
        rctx.file(path, rctx.read(path).replace(placeholder, relative_prefix(path)), executable = path.startswith("bin/"))
    rctx.file(package.license_file, rctx.read(info + "licenses/LICENSE"))

def _dist_impl(rctx):
    cell = OCAML_CELLS[rctx.attr.cell]
    for package in cell.packages:
        _install_package(rctx, package)
    rctx.delete("_conda")
    rctx.file(VERSION_FILE, OCAML_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _DIST_BUILD.format(
        emitters = repr(cell.emitters),
        licenses = repr([p.license_file for p in cell.packages]),
        version_file = VERSION_FILE,
    ))
    return rctx.repo_metadata(reproducible = True)

dist_repo = repository_rule(
    implementation = _dist_impl,
    doc = "Installs the pinned conda-forge OCaml packages of one cell.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Cell key in OCAML_CELLS."),
    },
)

def hub_build():
    """Returns the hub BUILD file: toolchains per cell and emitter, and the target cell aliases."""
    content = []
    for cell in sorted(OCAML_CELLS.keys()):
        for emitter in OCAML_CELLS[cell].emitters:
            exec_constraints = OCAML_CELLS[cell].exec_compatible_with + EMITTERS[emitter].exec_compatible_with
            target_constraints = OCAML_CELLS[cell].exec_compatible_with + EMITTERS[emitter].target_compatible_with
            content.append("\n".join([
                toolchain_target(
                    name = "{}_{}".format(cell, emitter),
                    exec_compatible_with = exec_constraints,
                    target_compatible_with = target_constraints,
                    toolchain = "@{}{}//:{}".format(DIST_PREFIX, cell, emitter),
                    toolchain_type = str(Label("@rules_ocaml//toolchain/type:std")),
                ),
                toolchain_target(
                    name = "{}_{}_profile".format(cell, emitter),
                    exec_compatible_with = exec_constraints,
                    target_compatible_with = target_constraints,
                    toolchain = "@{}{}//:{}_profile".format(DIST_PREFIX, cell, emitter),
                    toolchain_type = str(Label("@rules_ocaml//toolchain/type:profile")),
                ),
            ]))
    for cell in sorted(OCAML_CELLS.keys()):
        content.append(_CELL.format(cell = cell, constraints = repr(OCAML_CELLS[cell].exec_compatible_with)))
    content.append(_OCAMLRUN.format(cases = repr({":" + cell: "@" + DIST_PREFIX + cell + "//:ocamlrun" for cell in sorted(OCAML_CELLS.keys())})))
    content.append(_NONE)
    for alias in sorted(CELL_ALIASES.keys()):
        cases = {":" + cell: "@" + DIST_PREFIX + cell + CELL_ALIASES[alias] for cell in sorted(OCAML_CELLS.keys())}
        if alias in EMPTY_DEFAULT_ALIASES:
            cases["//conditions:default"] = ":none"
        content.append(_ALIAS.format(name = alias, cases = repr(cases)))
    return "\n".join(content)

def package_repo_name(group, package):
    """Returns the repository name of one opam package in a packages group."""
    return "dx_opam_{}_{}".format(group, package.replace("-", "_").replace(".", "_"))

def packages_hub_build(group, packages):
    """Returns the BUILD file of a packages hub with one alias per package."""
    return "\n".join([
        'alias(name = "{p}", actual = "@{repo}//:{p}", visibility = ["//visibility:public"])\n'.format(
            p = package,
            repo = package_repo_name(group, package),
        )
        for package in sorted(packages)
    ])

def dune_package_build(package, library, dune, rule, deps = [], hub = ""):
    """Returns the BUILD file that builds one dune package of an opam archive."""
    return """load("{rule}", "dx_dune_library")

package(default_visibility = ["//visibility:public"])

dx_dune_library(
    name = "{package}",
    srcs = glob(["**"], exclude = ["**/BUILD", "**/BUILD.bazel"]),
    deps = {deps},
    dune = "{dune}",
    library = "{library}",
    package = "{package}",
    workspace = "dune-project",
)
""".format(package = package, library = library, dune = dune, rule = rule, deps = repr(["@{}//:{}".format(hub, d) for d in deps]))

DUNE_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "srcs",
    srcs = glob(["**"], exclude = ["**/BUILD", "**/BUILD.bazel"]),
)
"""
