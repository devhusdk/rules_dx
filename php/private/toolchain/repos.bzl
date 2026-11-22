"""Hermetic PHP, Composer and PHPUnit acquisition and toolchain registration."""

load(":cells.bzl", "PHP_CELLS")

HUB_NAME = "dx_php_toolchains"

VERSION_FILE = "DX_PHP_VERSION"

TOOLCHAIN_TYPE = str(Label("//php:toolchain_type"))

EXEC_TOOLCHAIN_TYPE = str(Label("//php:exec_toolchain_type"))

_RUNTIME_BUILD = """load("{php_bzl}", "php_toolchain")

package(default_visibility = ["//visibility:public"])

exports_files(["{interpreter}", "{version_file}"])

php_toolchain(
    name = "toolchain",
    interpreter = "{interpreter}",
    version = "{version}",
)
"""

def runtime_build(version, interpreter = "php"):
    """Returns the BUILD file of one runtime repository."""
    return _RUNTIME_BUILD.format(
        interpreter = interpreter,
        php_bzl = str(Label("//php/private/toolchain:php.bzl")),
        version = version,
        version_file = VERSION_FILE,
    )

def runtime_repository_name(cell):
    """Returns the repository name one cell's runtime is acquired into."""
    return HUB_NAME + "__" + cell

_HUB_TOOLCHAIN = """toolchain(
    name = {name},
    {constraint} = {constraints},
    toolchain = {toolchain},
    toolchain_type = {toolchain_type},
    visibility = ["//visibility:public"],
)
"""

def hub_build(cells):
    """Returns the hub BUILD file: a runtime and an exec toolchain per cell."""
    parts = []
    for side, suffix, toolchain_type in [("target", "", TOOLCHAIN_TYPE), ("exec", "_exec", EXEC_TOOLCHAIN_TYPE)]:
        for cell in sorted(cells):
            repo = runtime_repository_name(cell)
            parts.append(_HUB_TOOLCHAIN.format(
                name = repr(repo + suffix),
                constraint = side + "_compatible_with",
                constraints = repr(list(PHP_CELLS[cell].exec_compatible_with)),
                toolchain = repr("@{}//:toolchain".format(repo)),
                toolchain_type = repr(toolchain_type),
            ))
    return "\n".join(parts)

SOURCES_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "files",
    srcs = glob(["src/**", "config/**", "rules.neon"], allow_empty = True),
)
"""

def phar_build(file):
    """Returns the BUILD file that exports one downloaded phar."""
    return 'package(default_visibility = ["//visibility:public"])\n\nexports_files([{}])\n'.format(repr(file))

def _hub_impl(rctx):
    rctx.file("REPO.bazel", "")
    for path, content in rctx.attr.files.items():
        rctx.file(path, content)
    return rctx.repo_metadata(reproducible = True)

php_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Writes rendered files into a repository that downloads nothing.",
    attrs = {
        "files": attr.string_dict(doc = "Repository path to file content."),
    },
)

def _archive_impl(rctx):
    if not rctx.attr.sha256:
        fail(rctx.name + ": set sha256; an archive without a digest is not pinned")
    if rctx.attr.download_as:
        rctx.download(url = rctx.attr.urls, output = rctx.attr.download_as, sha256 = rctx.attr.sha256)
    else:
        rctx.download_and_extract(
            url = rctx.attr.urls,
            sha256 = rctx.attr.sha256,
            stripPrefix = rctx.attr.strip_prefix,
            type = rctx.attr.type,
        )
    for inner in rctx.attr.inner:
        rctx.extract(inner)
        rctx.delete(inner)
    missing = [path for path in rctx.attr.required if not rctx.path(path).exists]
    if missing:
        fail("{} archive does not ship {}".format(rctx.name, ", ".join(missing)))
    rctx.file("REPO.bazel", "")
    for path, content in rctx.attr.files.items():
        rctx.file(path, content)
    for patch in rctx.attr.patches:
        rctx.patch(patch, strip = 1)
    return rctx.repo_metadata(reproducible = True)

php_archive = repository_rule(
    implementation = _archive_impl,
    doc = "Fetches one pinned archive, checks the required paths, writes rendered files and applies patches.",
    attrs = {
        "download_as": attr.string(doc = "File name to download to without extracting."),
        "files": attr.string_dict(doc = "Repository path to file content."),
        "inner": attr.string_list(doc = "Archives inside the downloaded one, extracted into the repository and then deleted."),
        "patches": attr.label_list(doc = "Patches applied last with strip 1."),
        "required": attr.string_list(doc = "Paths the archive must ship."),
        "sha256": attr.string(mandatory = True, doc = "Archive digest."),
        "strip_prefix": attr.string(doc = "Archive directory prefix to strip."),
        "type": attr.string(doc = "Archive type when the URL does not show it."),
        "urls": attr.string_list(mandatory = True, doc = "Archive URLs, tried in order."),
    },
)
