"""Hermetic OCaml acquisition and toolchain registration."""

load(
    ":pins.bzl",
    "OCAML_CELLS",
    "OCAML_VERSION",
    "RULES_OCAML_COMMIT",
    "RULES_OCAML_SHA256",
    "RULES_OCAML_URL",
)

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
        exec_compatible_with = ["@rules_ocaml//platform/emitter:sys"],
        target_compatible_with = ["@rules_ocaml//platform/executor:sys"],
    ),
    "vm": struct(
        exec_compatible_with = ["@rules_ocaml//platform/emitter:vm"],
        target_compatible_with = ["@rules_ocaml//platform/executor:vm"],
    ),
}

_DIST_BUILD = """load("@rules_dx//ocaml/toolchain:ocaml.bzl", "dx_ocaml_dist")

package(default_visibility = ["//visibility:public"])

exports_files(glob(["**"]))

dx_ocaml_dist(
    name = "dist",
    emitters = {emitters},
    licenses = {licenses},
    version_file = "{version_file}",
)
"""

_TOOLCHAINS = """toolchain(
    name = "{cell}_{emitter}",
    exec_compatible_with = {exec_constraints},
    target_compatible_with = {target_constraints},
    toolchain = "@{repo}//:{emitter}",
    toolchain_type = "@rules_ocaml//toolchain/type:std",
)

toolchain(
    name = "{cell}_{emitter}_profile",
    exec_compatible_with = {exec_constraints},
    target_compatible_with = {target_constraints},
    toolchain = "@{repo}//:{emitter}_profile",
    toolchain_type = "@rules_ocaml//toolchain/type:profile",
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

def _rules_ocaml_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        sha256 = rctx.attr.sha256,
        stripPrefix = "rules_ocaml-" + rctx.attr.commit,
    )
    for path in DROPPED_PATHS:
        rctx.delete(path)
    rctx.file("REPO.bazel", "")
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    return rctx.repo_metadata(reproducible = True)

_rules_ocaml = repository_rule(
    implementation = _rules_ocaml_impl,
    doc = "Fetches the pinned rules_ocaml sources with the hermetic OCaml patch.",
    attrs = {
        "commit": attr.string(mandatory = True, doc = "Pinned rules_ocaml commit."),
        "patches": attr.label_list(doc = "Patches applied after extraction."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "url": attr.string(mandatory = True, doc = "Archive URL."),
    },
)

def _install_package(rctx, package):
    stem = conda_stem(package.url)
    rctx.download(url = package.url, output = "_conda/" + stem + ".conda", sha256 = package.sha256)
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

_dist = repository_rule(
    implementation = _dist_impl,
    doc = "Installs the pinned conda-forge OCaml packages of one cell.",
    attrs = {
        "cell": attr.string(mandatory = True, doc = "Cell key in OCAML_CELLS."),
    },
)

def _hub_impl(rctx):
    content = []
    for cell in sorted(OCAML_CELLS.keys()):
        for emitter in OCAML_CELLS[cell].emitters:
            content.append(_TOOLCHAINS.format(
                cell = cell,
                emitter = emitter,
                exec_constraints = repr(OCAML_CELLS[cell].exec_compatible_with + EMITTERS[emitter].exec_compatible_with),
                target_constraints = repr(OCAML_CELLS[cell].exec_compatible_with + EMITTERS[emitter].target_compatible_with),
                repo = DIST_PREFIX + cell,
            ))
    for cell in sorted(OCAML_CELLS.keys()):
        content.append(_CELL.format(cell = cell, constraints = repr(OCAML_CELLS[cell].exec_compatible_with)))
    content.append(_OCAMLRUN.format(cases = repr({":" + cell: "@" + DIST_PREFIX + cell + "//:ocamlrun" for cell in sorted(OCAML_CELLS.keys())})))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", "\n".join(content))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares the OCaml toolchains per cell and emitter and the ocamlrun of the target cell.",
)

def _ocaml_impl(mctx):
    _rules_ocaml(
        name = "rules_ocaml",
        commit = RULES_OCAML_COMMIT,
        patches = [
            Label("//patches/rules_ocaml:hermetic_ocaml.patch"),
            Label("//patches/rules_ocaml:native_cc_env.patch"),
        ],
        sha256 = RULES_OCAML_SHA256,
        url = RULES_OCAML_URL,
    )
    for cell in sorted(OCAML_CELLS.keys()):
        _dist(name = DIST_PREFIX + cell, cell = cell)
    _hub(name = HUB_NAME)
    return mctx.extension_metadata(reproducible = True)

ocaml = module_extension(
    implementation = _ocaml_impl,
    doc = "Creates rules_ocaml, the pinned OCaml compilers and the toolchain hub.",
)
