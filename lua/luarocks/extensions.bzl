"""The luarocks module extension: one hub of rocks per luarocks.lock."""

load(":repos.bzl", "declare")

_lock = tag_class(
    doc = "One luarocks.lock and its digest file.",
    attrs = {
        "dev": attr.bool(doc = "Whether the hub is a development dependency of the root module."),
        "digests": attr.label(mandatory = True, allow_single_file = [".json"], doc = "luarocks.digests.json: {\"packages\": {\"rock\": {\"version\", \"source\": {\"urls\", \"sha256\", \"strip_prefix\"}, \"modules\": {\"module\": \"path\"}, \"deps\": [\"rock\"]}}}."),
        "lock": attr.label(mandatory = True, allow_single_file = [".lock"], doc = "The luarocks.lock LuaRocks wrote."),
        "name": attr.string(mandatory = True, doc = "The hub repository name."),
    },
)

def _luarocks_impl(module_ctx):
    root_hubs = []
    root_dev_hubs = []
    for mod in module_ctx.modules:
        for tag in mod.tags.lock:
            declare(tag.name, module_ctx.read(tag.lock), module_ctx.read(tag.digests))
            if mod.is_root:
                if tag.dev or module_ctx.is_dev_dependency(tag):
                    root_dev_hubs.append(tag.name)
                else:
                    root_hubs.append(tag.name)
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = root_hubs,
        root_module_direct_dev_deps = root_dev_hubs,
    )

luarocks = module_extension(
    doc = "Fetches every rock of a luarocks.lock through Bazel's repository cache; LuaRocks is never run at fetch time.",
    implementation = _luarocks_impl,
    tag_classes = {"lock": _lock},
)
