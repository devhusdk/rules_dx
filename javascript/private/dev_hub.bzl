"""Forwards a dev-only npm hub's link macro, or stubs it out for consumers.

rules_js only lets npm_link_all_packages run in the lockfile's own package.
rules_dx keeps its test lock at the repo root, and consumers load that root
package through @rules_dx//:dx. A direct load of the hub there would force
the hub to be non-dev and make every consumer fetch it. This shim loads the
real hub only when rules_dx is the root module. Otherwise its macro does
nothing, so consumers never fetch the hub or its link repos.
"""

def dev_hub_defs(hub, forward):
    """Returns the defs.bzl text for the shim repo.

    Args:
      hub: The apparent name of the dev hub repo.
      forward: Whether to load the real hub (root module) or stub it.

    Returns:
      The defs.bzl file content.
    """
    if forward:
        return "\n".join([
            '"""Forwards npm_link_all_packages from @{}."""'.format(hub),
            "",
            'load("@{}//:defs.bzl", _npm_link_all_packages = "npm_link_all_packages")'.format(hub),
            "",
            "npm_link_all_packages = _npm_link_all_packages",
            "",
        ])
    return "\n".join([
        '"""Stub for @{}: rules_dx is not the root module, so the dev hub is absent."""'.format(hub),
        "",
        'def npm_link_all_packages(name = "node_modules", **kwargs):',
        '    """Does nothing; the dev hub only exists when rules_dx is the root module."""',
        "    _ignored = (name, kwargs)",
        "",
    ])

def _link_repo_impl(rctx):
    rctx.file("BUILD.bazel", "")
    rctx.file("defs.bzl", dev_hub_defs(rctx.attr.hub, rctx.attr.forward))

_link_repo = repository_rule(
    implementation = _link_repo_impl,
    doc = "Writes a defs.bzl that forwards to a dev hub or stubs it out.",
    attrs = {
        "forward": attr.bool(doc = "Whether to load the real hub.", mandatory = True),
        "hub": attr.string(doc = "The apparent name of the dev hub repo.", mandatory = True),
    },
)

_link = tag_class(
    doc = "One shim repo that forwards a dev hub's link macro.",
    attrs = {
        "hub": attr.string(doc = "The apparent name of the dev hub repo.", mandatory = True),
        "name": attr.string(doc = "The shim repo name.", mandatory = True),
    },
)

def _dx_dev_hub_impl(module_ctx):
    for mod in module_ctx.modules:
        for tag in mod.tags.link:
            _link_repo(name = tag.name, hub = tag.hub, forward = mod.is_root)
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = "all",
        root_module_direct_dev_deps = [],
    )

dx_dev_hub = module_extension(
    doc = "Creates shim repos that load a dev hub only when the declaring module is root.",
    implementation = _dx_dev_hub_impl,
    tag_classes = {"link": _link},
)
