"""The Ruby module extension: the portable Ruby toolchain pinned in pins.json and gem hubs built from dx locks."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("@rules_ruby//ruby:deps.bzl", "rb_register_toolchains")
load("//core:pins.bzl", "lock_tag")
load(":gems.bzl", "ruby_gems_repos")

_PORTABLE_RUBY = DATA["dx_ruby"]["portable_ruby"]

RUBY_VERSION = _PORTABLE_RUBY["version"]

def _ruby_impl(module_ctx):
    direct = []
    direct_dev = []
    toolchains = []
    for mod in module_ctx.modules:
        for tag in mod.tags.lock:
            (direct_dev if module_ctx.is_dev_dependency(tag) else direct).append(ruby_gems_repos(module_ctx, tag))
        for toolchain in mod.tags.toolchain:
            if toolchain.name in toolchains:
                continue
            if not mod.is_root and toolchain.name != "ruby":
                fail("ruby.toolchain: only the root module may name the toolchain, got " + toolchain.name)
            toolchains.append(toolchain.name)
            names = [toolchain.name, "dx_%s_toolchains" % toolchain.name]
            (direct_dev if module_ctx.is_dev_dependency(toolchain) else direct).extend(names)
    for name in toolchains:
        rb_register_toolchains(
            name = name,
            version = RUBY_VERSION,
            portable_ruby = True,
            portable_ruby_checksums = _PORTABLE_RUBY["sha256"],
            resolved_version = RUBY_VERSION,
        )
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = direct,
        root_module_direct_dev_deps = direct_dev,
    )

ruby = module_extension(
    doc = "Registers the portable Ruby pinned in ruby/pins.json, and builds a gem hub from the dx lock each `lock` tag names.",
    implementation = _ruby_impl,
    tag_classes = {
        "lock": lock_tag,
        "toolchain": tag_class(
            doc = "The pinned portable Ruby toolchain, as @<name> and @dx_<name>_toolchains.",
            attrs = {
                "name": attr.string(default = "ruby", doc = "Base name of the toolchain repositories."),
            },
        ),
    },
)
