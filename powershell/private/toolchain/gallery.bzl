"""The powershell module extension and its pinned Gallery modules."""

load("//core:repo.bzl", "hub_repo")

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
    return ctx.repo_metadata(reproducible = True)

psgallery_module = repository_rule(
    implementation = _psgallery_module_impl,
    attrs = {
        "module": attr.string(mandatory = True, doc = "Module name and directory."),
        "sha256": attr.string(mandatory = True, doc = "SHA-256 of the package."),
        "urls": attr.string_list(mandatory = True, doc = "Package download URLs."),
    },
    doc = "Fetches one pinned module package into a PSModulePath layout.",
)

_HUB_ALIAS = """alias(
    name = "{name}",
    actual = "@{hub}_{name}//:{name}",
    visibility = ["//visibility:public"],
)
"""

def _powershell_impl(mctx):
    for tag in [tag for module in mctx.modules for tag in module.tags.lock]:
        lock = json.decode(mctx.read(tag.lock))
        aliases = []
        for module, entry in sorted(lock["modules"].items()):
            name = module.lower()
            psgallery_module(
                name = "{}_{}".format(tag.name, name),
                module = module,
                sha256 = entry["sha256"],
                urls = ["{}/package/{}/{}".format(lock["source"], module, entry["version"])],
            )
            aliases.append(_HUB_ALIAS.format(name = name, hub = tag.name))
        hub_repo(name = tag.name, files = {"BUILD.bazel": "\n".join(aliases)})
    return mctx.extension_metadata(reproducible = True)

powershell = module_extension(
    implementation = _powershell_impl,
    doc = "Creates a hub repository with one alias per module pinned in a Gallery lock file.",
    tag_classes = {
        "lock": tag_class(
            doc = "Creates the hub and one repository per pinned module.",
            attrs = {
                "lock": attr.label(mandatory = True, allow_single_file = [".json"], doc = "The Gallery lock file."),
                "name": attr.string(mandatory = True, doc = "Hub repository name."),
            },
        ),
    },
)
