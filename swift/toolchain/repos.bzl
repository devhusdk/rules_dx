"""Module extension that fetches the pinned Swift toolchains."""

load(":pins.bzl", "SWIFT_ARCHIVES")

_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files([
    ".swift-version",
    "usr/bin/swiftc",
    "usr/bin/swift-autolink-extract",
    "usr/bin/swift-symbolgraph-extract",
])

filegroup(
    name = "files",
    srcs = glob(["**/*"], exclude = ["BUILD.bazel", "REPO.bazel"]),
)
"""

def _swift_linux_impl(repository_ctx):
    version = repository_ctx.attr.swift_version
    platform = repository_ctx.attr.platform
    url = "https://download.swift.org/swift-{v}-release/{p}/swift-{v}-RELEASE/swift-{v}-RELEASE-{plat}.tar.gz".format(
        v = version,
        p = platform.replace(".", ""),
        plat = platform,
    )
    repository_ctx.download_and_extract(
        url = url,
        sha256 = repository_ctx.attr.sha256,
        stripPrefix = "swift-{}-RELEASE-{}".format(version, platform),
    )
    repository_ctx.file(".swift-version", version)
    repository_ctx.file("BUILD.bazel", _BUILD)
    return repository_ctx.repo_metadata(reproducible = True)

_swift_linux = repository_rule(
    implementation = _swift_linux_impl,
    attrs = {
        "platform": attr.string(mandatory = True),
        "sha256": attr.string(mandatory = True),
        "swift_version": attr.string(mandatory = True),
    },
)

def _swift_impl(module_ctx):
    for mod in module_ctx.modules:
        for tag in mod.tags.toolchain:
            if tag.swift_version not in SWIFT_ARCHIVES:
                fail("Unknown Swift version %s. Known: %s" % (tag.swift_version, SWIFT_ARCHIVES.keys()))
            for cpu, archive in SWIFT_ARCHIVES[tag.swift_version].items():
                _swift_linux(
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
