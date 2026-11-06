"""Hermetic OCaml acquisition and toolchain registration."""

load("//libs/starlark:repo.bzl", "hub_repo", "pinned_archive")
load(":opam.bzl", "is_toolchain_package", "parse_depend_names", "parse_depends", "parse_url")
load(
    ":pins.bzl",
    "DUNE_SHA256",
    "DUNE_URL",
    "DUNE_VERSION",
    "OCAML_CELLS",
    "OCAML_VERSION",
    "OPAM_REPOSITORY_SHA256",
    "OPAM_REPOSITORY_URL",
    "OPAM_REPOSITORY_COMMIT",
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
        exec_compatible_with = [str(Label("//ocaml/toolchain/platform/emitter:sys"))],
        target_compatible_with = [str(Label("//ocaml/toolchain/platform/executor:sys"))],
    ),
    "vm": struct(
        exec_compatible_with = [str(Label("//ocaml/toolchain/platform/emitter:vm"))],
        target_compatible_with = [str(Label("//ocaml/toolchain/platform/executor:vm"))],
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
    toolchain_type = "{std_type}",
)

toolchain(
    name = "{cell}_{emitter}_profile",
    exec_compatible_with = {exec_constraints},
    target_compatible_with = {target_constraints},
    toolchain = "@{repo}//:{emitter}_profile",
    toolchain_type = "{profile_type}",
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
    "tree": "//:tree",
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

def hub_build():
    """Returns the hub BUILD file: toolchains per cell and emitter, and the target cell aliases."""
    content = []
    for cell in sorted(OCAML_CELLS.keys()):
        for emitter in OCAML_CELLS[cell].emitters:
            content.append(_TOOLCHAINS.format(
                cell = cell,
                emitter = emitter,
                exec_constraints = repr(OCAML_CELLS[cell].exec_compatible_with + EMITTERS[emitter].exec_compatible_with),
                target_constraints = repr(OCAML_CELLS[cell].exec_compatible_with + EMITTERS[emitter].target_compatible_with),
                repo = DIST_PREFIX + cell,
                std_type = str(Label("//ocaml/toolchain/type:std")),
                profile_type = str(Label("//ocaml/toolchain/type:profile")),
            ))
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

_DUNE_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "srcs",
    srcs = glob(["**"], exclude = ["**/BUILD", "**/BUILD.bazel"]),
)
"""

def _snapshot_opam(mctx, package, version):
    path = "opam_snapshot/packages/{p}/{p}.{v}/opam".format(p = package, v = version)
    if not mctx.path(path).exists:
        fail("opam-repository {} has no {} {}".format(OPAM_REPOSITORY_COMMIT, package, version))
    return mctx.read(path)

def _declare_packages(mctx, tag, annotations):
    packages = {}
    locked = parse_depends(mctx.read(tag.lock))
    for package, version in locked.items():
        if is_toolchain_package(package):
            continue
        opam = _snapshot_opam(mctx, package, version)
        url = parse_url(opam)
        annotation = annotations.get(package)
        if annotation and annotation.build_file:
            build_file = mctx.read(annotation.build_file)
        else:
            needs = [n for n in parse_depend_names(opam) if not is_toolchain_package(n)]
            missing = [n for n in needs if n not in locked]
            if missing:
                fail("{} {} depends on {}, which {} does not lock".format(package, version, ", ".join(missing), tag.lock))
            library = annotation.library if annotation and annotation.library else package.replace("-", "_").replace(".", "_")
            build_file = dune_package_build(package, library, str(Label("//ocaml/toolchain:dune")), str(Label("//ocaml/rules:dune.bzl")), needs, tag.name)
        pinned_archive(
            name = package_repo_name(tag.name, package),
            files = {"BUILD.bazel": build_file},
            sha256 = url.sha256,
            strip_prefix = annotation.strip_prefix if annotation else "{}-{}".format(package, version),
            urls = [url.src],
        )
        packages[package] = version
    hub_repo(name = tag.name, files = {"BUILD.bazel": packages_hub_build(tag.name, packages)})

def _ocaml_impl(mctx):
    pinned_archive(
        name = "rules_ocaml",
        dropped = DROPPED_PATHS,
        patches = [
            Label("//patches/rules_ocaml:hermetic_ocaml.patch"),
            Label("//patches/rules_ocaml:native_cc_env.patch"),
            Label("//patches/rules_ocaml:platform_alias.patch"),
            Label("//patches/rules_ocaml:toolchain_type_alias.patch"),
        ],
        sha256 = RULES_OCAML_SHA256,
        strip_prefix = "rules_ocaml-" + RULES_OCAML_COMMIT,
        urls = [RULES_OCAML_URL],
    )
    for cell in sorted(OCAML_CELLS.keys()):
        _dist(name = DIST_PREFIX + cell, cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    pinned_archive(
        name = "dx_ocaml_dune",
        files = {"BUILD.bazel": _DUNE_BUILD},
        sha256 = DUNE_SHA256,
        strip_prefix = "dune-" + DUNE_VERSION,
        urls = [DUNE_URL],
    )
    tags = [tag for module in mctx.modules for tag in module.tags.packages]
    if tags:
        mctx.download_and_extract(
            url = [OPAM_REPOSITORY_URL],
            output = "opam_snapshot",
            sha256 = OPAM_REPOSITORY_SHA256,
            strip_prefix = "opam-repository-" + OPAM_REPOSITORY_COMMIT,
        )
    annotations = {tag.package: tag for module in mctx.modules for tag in module.tags.annotation}
    for tag in tags:
        _declare_packages(mctx, tag, annotations)
    return mctx.extension_metadata(reproducible = True)

ocaml = module_extension(
    implementation = _ocaml_impl,
    doc = "Creates rules_ocaml, the pinned OCaml compilers, the toolchain hub and opam package hubs.",
    tag_classes = {
        "annotation": tag_class(
            doc = "Adjusts how one opam package is built; without a build_file it is built with dune.",
            attrs = {
                "build_file": attr.label(allow_single_file = True, doc = "BUILD file placed in the package repository, for a package not built with dune."),
                "library": attr.string(doc = "The dune library name when it differs from the package name with - and . as _."),
                "package": attr.string(mandatory = True, doc = "Opam package name."),
                "strip_prefix": attr.string(doc = "Archive directory prefix to strip."),
            },
        ),
        "packages": tag_class(
            doc = "Creates a hub repository with one alias per package pinned in an opam lock file.",
            attrs = {
                "lock": attr.label(mandatory = True, allow_single_file = True, doc = "The `.opam.locked` file."),
                "name": attr.string(mandatory = True, doc = "Hub repository name."),
            },
        ),
    },
)
