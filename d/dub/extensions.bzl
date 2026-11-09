"""The dub module extension: one hub of packages per dub.selections.json."""

load(":repos.bzl", "declare")

_lock = tag_class(
    doc = "One dub.selections.json and its digest sidecar.",
    attrs = {
        "digests": attr.label(mandatory = True, allow_single_file = [".json"], doc = "dub.digests.json: {\"packages\": {\"name\": {\"version\", \"sha256\"}}}."),
        "name": attr.string(mandatory = True, doc = "The hub repository name."),
        "selections": attr.label(mandatory = True, allow_single_file = [".json"], doc = "The dub.selections.json DUB wrote."),
    },
)

def _dub_impl(module_ctx):
    root_hubs = []
    root_dev_hubs = []
    for mod in module_ctx.modules:
        for tag in mod.tags.lock:
            declare(tag.name, module_ctx.read(tag.selections), module_ctx.read(tag.digests))
            if mod.is_root:
                if module_ctx.is_dev_dependency(tag):
                    root_dev_hubs.append(tag.name)
                else:
                    root_hubs.append(tag.name)
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = root_hubs,
        root_module_direct_dev_deps = root_dev_hubs,
    )

dub = module_extension(
    doc = "Fetches every package of a dub.selections.json through Bazel's repository cache; DUB is never run at fetch time.",
    implementation = _dub_impl,
    tag_classes = {"lock": _lock},
)
