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

_SWIFT_WINDOWS_VERSION = "6.4.2"
_WINDOWS_CELL_CPU = {"aarch64": "arm64"}
_SWIFT_WINDOWS = {
    "x86_64": struct(
        url = "https://download.swift.org/swift-6.4.2-release/windows10/swift-6.4.2-RELEASE/swift-6.4.2-RELEASE-windows10.exe",
        sha256 = "8fe5d31ca1153dfe6be3a76de0b93347fd42a27599671e9f9ade23a3648e7184",
    ),
    "aarch64": struct(
        url = "https://download.swift.org/swift-6.4.2-release/windows10-arm64/swift-6.4.2-RELEASE/swift-6.4.2-RELEASE-windows10-arm64.exe",
        sha256 = "e4e5fdb6ca88b18ac430fbce3de35dd80e2249a84a484a02272d82d89f2c7969",
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
