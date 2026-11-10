"""The zig module extension: one hub of packages per build.zig.zon."""

load(":repos.bzl", "declare")

_lock = tag_class(
    doc = "One build.zig.zon and its digest sidecar.",
    attrs = {
        "digests": attr.label(mandatory = True, allow_single_file = [".json"], doc = "zig.digests.json: {\"packages\": {\"name\": {\"hash\", \"sha256\"}}}."),
        "lock": attr.label(mandatory = True, allow_single_file = [".zon"], doc = "The build.zig.zon whose .dependencies are fetched."),
        "name": attr.string(mandatory = True, doc = "The hub repository name."),
    },
)

def _zig_deps_impl(module_ctx):
    root_hubs = []
    root_dev_hubs = []
    for mod in module_ctx.modules:
        for tag in mod.tags.lock:
            declare(tag.name, module_ctx.read(tag.lock), module_ctx.read(tag.digests))
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

zig_deps = module_extension(
    doc = "Fetches every .dependencies archive of a build.zig.zon through Bazel's repository cache; Zig is never run at fetch time.",
    implementation = _zig_deps_impl,
    tag_classes = {"lock": _lock},
)
