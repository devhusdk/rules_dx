"""Hermetic Gleam acquisition and toolchain registration."""

load("//erlang/toolchain:pins.bzl", "CELLS")
load("//gleam/rules:hex.bzl", "gleam_hex_package")
load(
    ":pins.bzl",
    "GLEAM_CELLS",
    "GLEAM_RELEASES",
    "GLEAM_STDLIB_SHA256",
    "GLEAM_STDLIB_VERSION",
    "GLEAM_VERSION",
    "GLEEUNIT_SHA256",
    "GLEEUNIT_VERSION",
    "RULES_GLEAM_COMMIT",
    "RULES_GLEAM_SHA256",
    "RULES_GLEAM_URL",
)

HUB_NAME = "dx_gleam_toolchains"

COMPILER_PREFIX = "dx_gleam_"

VERSION_FILE = "DX_GLEAM_VERSION"

DROPPED_PATHS = [
    ".bazelignore",
    ".bazelrc",
    ".bcr",
    ".github",
    ".gitignore",
    "BUILD",
    "MODULE.bazel",
    "MODULE.bazel.lock",
    "README.md",
    "examples",
    "extensions.bzl",
    "gazelle",
    "gleam/gleeunit",
    "gleam/templates/[[main_module]].sh",
    "gleam_hex",
    "gleam_tools/coded_versions.bzl",
    "gleam_tools/platforms.bzl",
    "gleam_tools/repositories.bzl",
    "go.mod",
    "go.sum",
    "internal",
]

_COMPILER_BUILD = """load("@rules_dx//gleam/toolchain:gleam.bzl", "dx_gleam_erlang_toolchain")
load("@rules_gleam//gleam_tools:toolchain.bzl", "gleam_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["gleam", "{version_file}"])

gleam_toolchain(
    name = "gleam_toolchain",
    compiler = "gleam",
)

dx_gleam_erlang_toolchain(name = "erlang_toolchain")
"""

_TOOLCHAINS = """toolchain(
    name = "{cell}",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:gleam_toolchain",
    toolchain_type = "@rules_gleam//gleam_tools:toolchain_type",
)

toolchain(
    name = "{cell}_erlang",
    exec_compatible_with = {constraints},
    target_compatible_with = {constraints},
    toolchain = "@{repo}//:erlang_toolchain",
    toolchain_type = "@rules_gleam//gleam_tools:erlang_toolchain_type",
)
"""

def _rules_gleam_impl(rctx):
    rctx.download_and_extract(
        url = rctx.attr.url,
        sha256 = rctx.attr.sha256,
        stripPrefix = "rules_gleam-" + rctx.attr.commit,
    )
    for path in DROPPED_PATHS:
        rctx.delete(path)
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", "")
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    return rctx.repo_metadata(reproducible = True)

_rules_gleam = repository_rule(
    implementation = _rules_gleam_impl,
    doc = "Fetches the pinned rules_gleam sources with the hermetic Gleam patch.",
    attrs = {
        "commit": attr.string(mandatory = True, doc = "Pinned rules_gleam commit."),
        "patches": attr.label_list(doc = "Patches applied after extraction."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "url": attr.string(mandatory = True, doc = "Archive URL."),
    },
)

def _compiler_impl(rctx):
    rctx.download_and_extract(url = GLEAM_RELEASES + rctx.attr.archive, sha256 = rctx.attr.sha256)
    if not rctx.path("gleam").exists:
        fail("Gleam archive " + rctx.attr.archive + " has no gleam binary")
    rctx.file(VERSION_FILE, GLEAM_VERSION + "\n")
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", _COMPILER_BUILD.format(version_file = VERSION_FILE))
    return rctx.repo_metadata(reproducible = True)

_compiler = repository_rule(
    implementation = _compiler_impl,
    doc = "Acquires one pinned Gleam compiler release.",
    attrs = {
        "archive": attr.string(mandatory = True, doc = "Release archive name."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
    },
)

def _hub_impl(rctx):
    content = []
    for cell in sorted(GLEAM_CELLS.keys()):
        content.append(_TOOLCHAINS.format(
            cell = cell,
            constraints = repr(CELLS[cell].exec_compatible_with),
            repo = COMPILER_PREFIX + cell,
        ))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", "\n".join(content))
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Declares the Gleam compiler and Erlang toolchains per qualified cell.",
)

def _gleam_impl(mctx):
    _rules_gleam(
        name = "rules_gleam",
        commit = RULES_GLEAM_COMMIT,
        patches = [Label("//patches/rules_gleam:hermetic_gleam.patch")],
        sha256 = RULES_GLEAM_SHA256,
        url = RULES_GLEAM_URL,
    )
    for cell in sorted(GLEAM_CELLS.keys()):
        _compiler(
            name = COMPILER_PREFIX + cell,
            archive = GLEAM_CELLS[cell].archive,
            sha256 = GLEAM_CELLS[cell].sha256,
        )
    _hub(name = HUB_NAME)
    gleam_hex_package(
        name = "dx_gleam_stdlib",
        package = "gleam_stdlib",
        sha256 = GLEAM_STDLIB_SHA256,
        version = GLEAM_STDLIB_VERSION,
    )
    gleam_hex_package(
        name = "dx_gleeunit",
        deps = ["@dx_gleam_stdlib//:gleam_stdlib"],
        package = "gleeunit",
        sha256 = GLEEUNIT_SHA256,
        version = GLEEUNIT_VERSION,
    )
    return mctx.extension_metadata(reproducible = True)

gleam = module_extension(
    implementation = _gleam_impl,
    doc = "Creates rules_gleam, the pinned Gleam compilers, stdlib, gleeunit and the toolchain hub.",
)
