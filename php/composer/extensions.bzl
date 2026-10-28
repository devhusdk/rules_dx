"""The composer module extension: one hub of packages per composer.lock."""

load(":repos.bzl", "declare")

_lock = tag_class(
    doc = "One composer.lock and its digest sidecar.",
    attrs = {
        "dev": attr.bool(doc = "Set when the extension is used with dev_dependency = True; the hub is then a dev dependency of the root module."),
        "digests": attr.label(mandatory = True, allow_single_file = [".json"], doc = "composer.digests.json: {\"version\": 1, \"packages\": {\"vendor/name\": {\"reference\", \"sha256\", \"urls\"?}}}."),
        "lock": attr.label(mandatory = True, allow_single_file = [".lock"], doc = "The composer.lock Composer wrote."),
        "name": attr.string(mandatory = True, doc = "The hub repository name."),
    },
)

def _composer_impl(module_ctx):
    root_hubs = []
    root_dev_hubs = []
    for mod in module_ctx.modules:
        for tag in mod.tags.lock:
            declare(tag.name, module_ctx.read(tag.lock), module_ctx.read(tag.digests))
            if mod.is_root:
                (root_dev_hubs if tag.dev else root_hubs).append(tag.name)
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = root_hubs,
        root_module_direct_dev_deps = root_dev_hubs,
    )

composer = module_extension(
    doc = "Fetches every package of a composer.lock through Bazel's repository cache; Composer is never run at fetch time.",
    implementation = _composer_impl,
    tag_classes = {"lock": _lock},
)
