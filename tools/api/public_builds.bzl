"""Narrows a set of source files to the `BUILD.bazel` and entry `.bzl` files of public packages."""

visibility("private")

_NON_PUBLIC_SEGMENTS = ("private", "tests")

def _is_public_package(package):
    segments = package.split("/") if package else []
    if segments and segments[0] == "tools":
        return False
    return not any([segment in _NON_PUBLIC_SEGMENTS for segment in segments])

def _public_builds_impl(ctx):
    files = [
        file
        for file in ctx.files.srcs
        if file.basename == "BUILD.bazel" and
           file.short_path == (file.owner.package + "/BUILD.bazel").removeprefix("/") and
           _is_public_package(file.owner.package)
    ]
    return [DefaultInfo(files = depset(files))]

public_builds = rule(
    implementation = _public_builds_impl,
    doc = "Passes on only the `BUILD.bazel` file of each package in `srcs` outside `private/`, `tests/` and `tools/`.",
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "Targets whose files include the package `BUILD.bazel` files.",
        ),
    },
)

_PUBLIC_BZL_BASENAMES = ("defs.bzl", "extensions.bzl")

def _public_bzl_impl(ctx):
    files = [
        file
        for file in ctx.files.srcs
        if file.basename in _PUBLIC_BZL_BASENAMES and
           file.owner.package and
           "/" not in file.owner.package and
           file.short_path == file.owner.package + "/" + file.basename and
           _is_public_package(file.owner.package)
    ]
    return [DefaultInfo(files = depset(files))]

public_bzl = rule(
    implementation = _public_bzl_impl,
    doc = "Passes on only the `defs.bzl` and `extensions.bzl` files of the top-level packages in `srcs` outside `tools/`.",
    attrs = {
        "srcs": attr.label_list(
            allow_files = True,
            doc = "Targets whose files include the public `.bzl` files.",
        ),
    },
)
