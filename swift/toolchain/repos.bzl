"""Module extension for the host `swift` command."""

load("//libs/starlark:host.bzl", "host_cell")

# Libraries swift-package loads that a plain host may lack.
_HOST_LIBS = ["libxml2.so.2", "libicuuc.so.70", "libicudata.so.70"]

def _swift_host_impl(repository_ctx):
    host = host_cell(repository_ctx)
    if host.startswith("macos"):
        script = 'exec xcrun swift "$@"'
    else:
        arch = "aarch64" if host.endswith("arm64") else "x86_64"
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

def _swift_host_ext_impl(module_ctx):
    _swift_host(name = "dx_swift_host")
    return module_ctx.extension_metadata(reproducible = True)

swift_host = module_extension(implementation = _swift_host_ext_impl)

# swift-format is not published as a release binary; the Homebrew bottle built
# from the swiftlang/swift-format 604.0.0 tag is the pinned macOS arm64 build.
# ghcr.io wants a bearer token even for public blobs, so the rule fetches an
# anonymous pull token first; the blob itself is pinned by sha256.
_SWIFT_FORMAT_VERSION = "604.0.0"
_SWIFT_FORMAT_MACOS_ARM64_SHA256 = "a4890bc578594b94782ec0e434097a467a1a8e94271b504e0430326a6db55fb2"

def _swift_format_macos_impl(repository_ctx):
    repository_ctx.download(
        url = "https://ghcr.io/token?service=ghcr.io&scope=repository:homebrew/core/swift-format:pull",
        output = "token.json",
    )
    token = json.decode(repository_ctx.read("token.json"))["token"]
    repository_ctx.delete("token.json")
    repository_ctx.download_and_extract(
        url = "https://ghcr.io/v2/homebrew/core/swift-format/blobs/sha256:" + _SWIFT_FORMAT_MACOS_ARM64_SHA256,
        sha256 = _SWIFT_FORMAT_MACOS_ARM64_SHA256,
        type = "tar.gz",
        stripPrefix = "swift-format/" + _SWIFT_FORMAT_VERSION,
        headers = {"Authorization": ["Bearer " + token]},
    )
    repository_ctx.file("BUILD.bazel", 'exports_files(["bin/swift-format"], visibility = ["//visibility:public"])\n')

_swift_format_macos = repository_rule(
    implementation = _swift_format_macos_impl,
    doc = "The sha256-pinned swift-format macOS arm64 binary.",
)

def _swift_format_macos_ext_impl(module_ctx):
    _swift_format_macos(name = "dx_swift_format_macos_arm64")
    return module_ctx.extension_metadata(reproducible = True)

swift_format_macos = module_extension(implementation = _swift_format_macos_ext_impl)
