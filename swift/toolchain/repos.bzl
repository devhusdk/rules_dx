"""Module extension that fetches the pinned Swift toolchains."""

load(":pins.bzl", "SWIFT_ARCHIVES")

_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files([
    ".swift-version",
    "usr/bin/swift",
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

# Libraries swift-package loads that a plain host may lack.
_HOST_LIBS = ["libxml2.so.2", "libicuuc.so.70", "libicudata.so.70"]

def _swift_host_impl(repository_ctx):
    if repository_ctx.os.name.startswith("mac"):
        script = 'exec xcrun swift "$@"'
    else:
        arch = "aarch64" if repository_ctx.os.arch in ["aarch64", "arm64"] else "x86_64"
        swift = repository_ctx.path(Label("@dx_swift_linux_{}//:usr/bin/swift".format(arch)))
        sysroot = repository_ctx.path(Label("@dx_swift_sysroot_linux_{}//:REPO.bazel".format(arch))).dirname
        host_libs = sysroot.get_child("host_libs")
        for lib in _HOST_LIBS:
            repository_ctx.symlink(sysroot.get_child("usr/lib/{}-linux-gnu/{}".format(arch, lib)), "libs/" + lib)
        libs = repository_ctx.path("libs")
        script = 'LD_LIBRARY_PATH="{libs}:{host_libs}${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}" exec "{swift}" "$@"'.format(
            libs = libs,
            host_libs = host_libs,
            swift = swift,
        )
    repository_ctx.file("swift", "#!/bin/sh\n{}\n".format(script), executable = True)
    repository_ctx.file("BUILD.bazel", 'exports_files(["swift"], visibility = ["//visibility:public"])\n')

_swift_host = repository_rule(
    implementation = _swift_host_impl,
    doc = "A `swift` command that runs the managed toolchain on Linux and the Xcode one on macOS.",
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
    _swift_host(name = "dx_swift_host")
    return module_ctx.extension_metadata(reproducible = True)

swift = module_extension(
    implementation = _swift_impl,
    tag_classes = {
        "toolchain": tag_class(attrs = {"swift_version": attr.string(mandatory = True)}),
    },
)
