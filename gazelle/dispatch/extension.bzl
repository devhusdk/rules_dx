"""Creates @dx_gazelle: one gazelle_binary of the registered languages plus dispatch."""

load("//libs/starlark:repo.bzl", "hub_repo")
load(":hub.bzl", "render_hub_build")

_language = tag_class(
    doc = "Adds a Gazelle language package to the dx gazelle binary.",
    attrs = {
        "library": attr.label(doc = "The go_library of the language.", mandatory = True),
    },
)

def _dx_gazelle_impl(module_ctx):
    labels = [tag.library for mod in module_ctx.modules for tag in mod.tags.language]
    hub_repo(name = "dx_gazelle", files = {"BUILD.bazel": render_hub_build(labels)})
    return module_ctx.extension_metadata(reproducible = True)

dx_gazelle = module_extension(
    doc = "Creates @dx_gazelle, the gazelle binary behind @rules_dx//dx:generate.",
    implementation = _dx_gazelle_impl,
    tag_classes = {"language": _language},
)
