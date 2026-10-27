"""Dev-only CPAN hubs for rules_dx's own Perl examples."""

load("@rules_perl//perl/cpan/private:carton.bzl", "install")

_install = tag_class(
    doc = "One CPAN hub built from a carton lock.",
    attrs = {
        "lock": attr.label(
            doc = "The Bazel lock generated from cpanfile.snapshot.",
            allow_files = [".json"],
            mandatory = True,
        ),
        "name": attr.string(
            doc = "The hub repository name.",
            mandatory = True,
        ),
    },
)

def _cpan_dev_impl(module_ctx):
    hubs = []
    for mod in module_ctx.modules:
        for attrs in mod.tags.install:
            hub = install(
                module_ctx = module_ctx,
                attrs = struct(
                    name = attrs.name,
                    lock = attrs.lock,
                    add_deps = {},
                    bins = {},
                ),
            )
            if mod.is_root:
                hubs.append(hub)
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = [],
        root_module_direct_dev_deps = hubs,
    )

cpan_dev = module_extension(
    doc = "Builds CPAN hubs through rules_perl and reports them as dev dependencies.",
    implementation = _cpan_dev_impl,
    tag_classes = {"install": _install},
)
