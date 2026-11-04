"""Module extension that fetches the pinned Swift toolchains."""

load("@rules_swift//swift/internal/extensions:standalone_toolchain.bzl", "standalone_toolchain")
load(":pins.bzl", "SWIFT_ARCHIVES")

def _swift_impl(module_ctx):
    for mod in module_ctx.modules:
        for tag in mod.tags.toolchain:
            if tag.swift_version not in SWIFT_ARCHIVES:
                fail("Unknown Swift version %s. Known: %s" % (tag.swift_version, SWIFT_ARCHIVES.keys()))
            for cpu, archive in SWIFT_ARCHIVES[tag.swift_version].items():
                standalone_toolchain(
                    name = "dx_swift_linux_" + cpu,
                    platform = archive.platform,
                    sha256 = archive.sha256,
                    swift_version = tag.swift_version,
                )
    return module_ctx.extension_metadata(reproducible = True)

swift = module_extension(
    implementation = _swift_impl,
    tag_classes = {
        "toolchain": tag_class(attrs = {"swift_version": attr.string(mandatory = True)}),
    },
)
