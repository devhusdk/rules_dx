"""Module extension for the host `swift` command."""

load("//core:host.bzl", "host_cell")

def _swift_host_impl(repository_ctx):
    host = host_cell(repository_ctx)
    if host.startswith("macos"):
        script = 'exec xcrun swift "$@"'
    else:
        arch = "aarch64" if host.endswith("arm64") else "x86_64"
        swift = repository_ctx.path(Label("@dx_swift_linux_{}//:usr/bin/swift".format(arch)))
        sysroot = repository_ctx.path(Label("@dx_swift_sysroot_linux_{}//:REPO.bazel".format(arch))).dirname
        host_libs = sysroot.get_child("host_libs")
        script = 'LD_LIBRARY_PATH="{host_libs}${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}" exec "{swift}" "$@"'.format(
            host_libs = host_libs,
            swift = swift,
        )
    repository_ctx.file("swift", "#!/bin/sh\n{}\n".format(script), executable = True)
    repository_ctx.file("BUILD.bazel", 'exports_files(["swift"], visibility = ["//visibility:public"])\n')

swift_host_repo = repository_rule(
    implementation = _swift_host_impl,
    doc = "A `swift` command that runs the managed toolchain on Linux and the Xcode one on macOS.",
)

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

swift_format_macos_repo = repository_rule(
    implementation = _swift_format_macos_impl,
    doc = "The sha256-pinned swift-format macOS arm64 binary.",
)

_SWIFT_WINDOWS_VERSION = "6.4.1"
_WINDOWS_CELL_CPU = {"aarch64": "arm64"}
_SWIFT_WINDOWS = {
    "x86_64": struct(
        url = "https://download.swift.org/swift-6.4.1-release/windows10/swift-6.4.1-RELEASE/swift-6.4.1-RELEASE-windows10.exe",
        sha256 = "9382e14919eeae5e7b5dbfe0575452fe2154178e089a595c8b44d03960d91d59",
    ),
    "aarch64": struct(
        url = "https://download.swift.org/swift-6.4.1-release/windows10-arm64/swift-6.4.1-RELEASE/swift-6.4.1-RELEASE-windows10-arm64.exe",
        sha256 = "faff6a12cdae099d76372ca971b0c83b13280ad704e1022faa599dbe0b146e64",
    ),
}

_SWIFT_WINDOWS_BUILD = """load("@rules_swift//swift/toolchains:swift_tools.bzl", "swift_tools")

package(default_visibility = ["//visibility:public"])

exports_files([".swift-version"])

filegroup(
    name = "runtime_dlls",
    srcs = glob(
        [
            "runtime/usr/bin/*.dll",
            "platform/**/*.dll",
        ],
        allow_empty = True,
    ),
)

swift_tools(
    name = "tools",
    additional_inputs = glob(
        [
            "toolchain/**",
            "runtime/**",
            "platform/**",
        ],
        exclude = ["**/*.pdb"],
        allow_empty = True,
    ),
    swift_autolink_extract = "toolchain/usr/bin/swiftc.exe",
    swift_driver = "toolchain/usr/bin/swiftc.exe",
    swift_symbolgraph_extract = "toolchain/usr/bin/swift-symbolgraph-extract.exe",
)
"""

def _swift_windows_impl(repository_ctx):
    cpu = repository_ctx.attr.cpu
    pin = _SWIFT_WINDOWS[cpu]
    repository_ctx.file("BUILD.bazel", _SWIFT_WINDOWS_BUILD)
    repository_ctx.file(".swift-version", _SWIFT_WINDOWS_VERSION + "\n")
    cell = "windows_" + _WINDOWS_CELL_CPU.get(cpu, cpu)
    if host_cell(repository_ctx) != cell:
        for path in ["toolchain/usr/bin/swiftc.exe", "toolchain/usr/bin/swift-symbolgraph-extract.exe"]:
            repository_ctx.file(path, "")
        return
    repository_ctx.download(
        url = pin.url,
        output = "swift.exe",
        sha256 = pin.sha256,
        canonical_id = "swift-windows-" + _SWIFT_WINDOWS_VERSION + "-" + cpu,
    )
    pwsh = repository_ctx.path(Label("@dx_powershell_toolchains__{}//:pwsh.exe".format(cell)))
    script = repository_ctx.path(Label(":extract_windows.ps1"))
    result = repository_ctx.execute(
        [
            pwsh,
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script,
            "-Exe",
            repository_ctx.path("swift.exe"),
            "-Out",
            repository_ctx.path("."),
            "-Work",
            repository_ctx.path("_work"),
            "-XCTestVersion",
            _SWIFT_WINDOWS_VERSION,
        ],
        timeout = 3600,
    )
    if result.return_code != 0:
        fail("extracting the Swift Windows installer failed ({}):\n{}\n{}".format(result.return_code, result.stdout, result.stderr))
    repository_ctx.delete("swift.exe")

swift_windows_repo = repository_rule(
    implementation = _swift_windows_impl,
    attrs = {"cpu": attr.string(mandatory = True, values = ["x86_64", "aarch64"])},
    doc = "The sha256-pinned swift.org Windows toolchain, unpacked on a Windows host and stubbed elsewhere.",
)
