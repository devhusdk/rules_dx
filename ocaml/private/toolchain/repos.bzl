"""Hermetic OCaml acquisition and toolchain registration."""

load("//core:toolchain.bzl", "cell_repo", "pins_hub")
load(":pins.bzl", "OCAML_CELLS", "OCAML_VERSION")

HUB_NAME = "dx_ocaml_toolchains"

VERSION_FILE = "DX_OCAML_VERSION"

EXEC_TOOLCHAIN_TYPE = str(Label("//ocaml/private/toolchain:exec_toolchain_type"))

EMITTERS = {
    "sys": struct(
        exec_compatible_with = [str(Label("//ocaml/private/toolchain/platform/emitter:sys"))],
        target_compatible_with = [str(Label("//ocaml/private/toolchain/platform/executor:sys"))],
    ),
    "vm": struct(
        exec_compatible_with = [str(Label("//ocaml/private/toolchain/platform/emitter:vm"))],
        target_compatible_with = [str(Label("//ocaml/private/toolchain/platform/executor:vm"))],
    ),
}

_DIST_BUILD = """load("@rules_dx//ocaml/private/toolchain:ocaml.bzl", "dx_ocaml_dist")
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
        "@rules_dx//ocaml/private/toolchain:emitter_sys": [
            "lib/ocaml/libunixnat.a",
            "lib/ocaml/unix/unix.a",
        ],
        "//conditions:default": [],
    }}),
    archive = select({{
        "@rules_dx//ocaml/private/toolchain:emitter_sys": "lib/ocaml/unix/unix.cmxa",
        "//conditions:default": "lib/ocaml/unix/unix.cma",
    }}),
    sigs = glob(["lib/ocaml/unix/*.cmi", "lib/ocaml/unix/*.cmx"]),
)

ocaml_import(
    name = "str",
    afiles = select({{
        "@rules_dx//ocaml/private/toolchain:emitter_sys": [
            "lib/ocaml/libcamlstrnat.a",
            "lib/ocaml/str/str.a",
        ],
        "//conditions:default": [],
    }}),
    archive = select({{
        "@rules_dx//ocaml/private/toolchain:emitter_sys": "lib/ocaml/str/str.cmxa",
        "//conditions:default": "lib/ocaml/str/str.cma",
    }}),
    sigs = glob(["lib/ocaml/str/*.cmi", "lib/ocaml/str/*.cmx"]),
)
"""

# Aliases that resolve to an empty filegroup on exec platforms without an OCaml cell.
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

def softlink_paths(paths):
    """Returns the softlink entries of a conda paths.json."""
    return sorted([entry["_path"] for entry in paths if entry.get("path_type") == "softlink"])

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
    root = str(rctx.path("."))
    for path in softlink_paths(paths):
        target = rctx.path(path).realpath
        real = str(target)
        if not real.startswith(root + "/") or target.is_dir:
            continue
        content = rctx.read(real, watch = "no")
        rctx.delete(path)
        rctx.file(path, content, executable = path.startswith("bin/"))
    missing = [entry["_path"] for entry in paths if not rctx.path(entry["_path"]).exists]
    if missing:
        fail("OCaml package " + package.name + " is missing " + repr(missing) + " after extraction")
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

def dist_repository_name(cell):
    """Returns the repository name one cell's OCaml distribution is installed into."""
    return cell_repo(HUB_NAME, cell)

def hub_build():
    """Returns the hub BUILD file: toolchains per cell and emitter, and the target cell aliases."""
    aliases = {"ocamlrun": "//:ocamlrun"} | {alias: CELL_ALIASES[alias] for alias in sorted(CELL_ALIASES)}
    return "\n".join([
        pins_hub(
            HUB_NAME,
            OCAML_CELLS.keys(),
            str(Label("//ocaml:toolchain_type")),
            "both",
            aliases,
            variants = EMITTERS,
            companion_types = {"profile": str(Label("//ocaml:profile_toolchain_type"))},
            alias_defaults = {alias: ":none" for alias in EMPTY_DEFAULT_ALIASES},
            exec_toolchain_type = EXEC_TOOLCHAIN_TYPE,
            target = "vm",
        ),
        _NONE,
    ])

def dune_package_build(package, library, dune, rule, deps = []):
    """Returns the BUILD file that builds one dune package of an opam archive."""
    return """load("{rule}", "ocaml_dune_library")

package(default_visibility = ["//visibility:public"])

ocaml_dune_library(
    name = "{package}",
    srcs = glob(["**"], exclude = ["**/BUILD", "**/BUILD.bazel"]),
    deps = {deps},
    dune = "{dune}",
    library = "{library}",
    package = "{package}",
    workspace = "dune-project",
)
""".format(package = package, library = library, dune = dune, rule = rule, deps = repr(["//{}".format(d) for d in deps]))
