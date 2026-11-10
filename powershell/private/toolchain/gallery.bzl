"""Pinned PowerShell Gallery modules."""

_BUILD = """load("@rules_powershell//powershell:defs.bzl", "pwsh_library")

package(default_visibility = ["//visibility:public"])

pwsh_library(
    name = "{name}",
    srcs = [
        "{module}/{module}.psd1",
        "{module}/{module}.psm1",
    ],
    data = glob(
        ["{module}/**"],
        exclude = [
            "{module}/{module}.psd1",
            "{module}/{module}.psm1",
        ],
    ),
)
"""

_PACKAGING = ["_rels", "package", "[Content_Types].xml", "%5BContent_Types%5D.xml", ".signature.p7s"]

def _psgallery_module_impl(ctx):
    ctx.download_and_extract(
        url = ctx.attr.urls,
        sha256 = ctx.attr.sha256,
        type = "zip",
        output = ctx.attr.module,
    )
    for name in _PACKAGING:
        path = ctx.path(ctx.attr.module + "/" + name)
        if path.exists:
            ctx.delete(path)
    ctx.file("BUILD.bazel", _BUILD.format(name = ctx.attr.module.lower(), module = ctx.attr.module))

psgallery_module = repository_rule(
    implementation = _psgallery_module_impl,
    attrs = {
        "module": attr.string(mandatory = True, doc = "Module name and directory."),
        "sha256": attr.string(mandatory = True, doc = "SHA-256 of the package."),
        "urls": attr.string_list(mandatory = True, doc = "Package download URLs."),
    },
    doc = "Fetches one pinned module package into a PSModulePath layout.",
)
