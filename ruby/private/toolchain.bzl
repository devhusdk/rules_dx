"""The Ruby module extension: the portable Ruby toolchain pinned in pins.json and gem bundles fetched from the Bundler lock."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("@rules_ruby//ruby:deps.bzl", "rb_bundle_fetch", "rb_register_toolchains")

_PORTABLE_RUBY = DATA["dx_ruby"]["portable_ruby"]

RUBY_VERSION = _PORTABLE_RUBY["version"]

def _ruby_impl(module_ctx):
    direct = []
    direct_dev = []
    toolchains = []
    for mod in module_ctx.modules:
        for bundle in mod.tags.bundle_fetch:
            rb_bundle_fetch(
                name = bundle.name,
                srcs = bundle.srcs,
                env = bundle.env,
                gemfile = bundle.gemfile,
                gemfile_lock = bundle.gemfile_lock,
            )
            (direct_dev if module_ctx.is_dev_dependency(bundle) else direct).append(bundle.name)
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
            register = False,
        )
    return module_ctx.extension_metadata(
        reproducible = True,
        root_module_direct_deps = direct,
        root_module_direct_dev_deps = direct_dev,
    )

ruby = module_extension(
    doc = "Registers the portable Ruby pinned in ruby/pins.json and fetches gem bundles whose sha256s come from the CHECKSUMS section of the Bundler lock.",
    implementation = _ruby_impl,
    tag_classes = {
        "bundle_fetch": tag_class(
            doc = "A gem bundle repository made from a Bundler manifest and its lock.",
            attrs = {
                "env": attr.string_dict(doc = "Environment variables for `bundle install`."),
                "gemfile": attr.label(mandatory = True, doc = "The Bundler manifest."),
                "gemfile_lock": attr.label(mandatory = True, doc = "The Bundler lock with a CHECKSUMS section."),
                "name": attr.string(default = "bundle", doc = "Name of the bundle repository."),
                "srcs": attr.label_list(doc = "Files the Bundler manifest reads."),
            },
        ),
        "toolchain": tag_class(
            doc = "The pinned portable Ruby toolchain, as @<name> and @dx_<name>_toolchains.",
            attrs = {
                "name": attr.string(default = "ruby", doc = "Base name of the toolchain repositories."),
            },
        ),
    },
)
