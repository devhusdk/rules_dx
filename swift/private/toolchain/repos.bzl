"""Module extension for the host `swift` command."""

load("//core:host.bzl", "host_cell")
load("//core:seven_zip.bzl", "seven_zip_extract", "seven_zip_fetch")

def _swift_host_impl(repository_ctx):
    host = host_cell(repository_ctx)
    if host.startswith("macos"):
        script = 'exec xcrun swift "$@"'
    else:
        arch = "aarch64" if host.endswith("arm64") else "x86_64"
        swift = repository_ctx.path(Label("@dx_swift_linux_{}//:usr/bin/swift".format(arch)))
        sysroot = repository_ctx.path(Label("@dx_swift_sysroot_linux_{}//:REPO.bazel".format(arch))).dirname
        host_libs = sysroot.get_child("host_libs")
        repository_ctx.symlink(repository_ctx.path(Label("@dx_swift_linux_{}//:usr/bin/ld.lld".format(arch))), "linker/ld")
        script = 'COMPILER_PATH="{linker}" LD_LIBRARY_PATH="{host_libs}${{LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}}" exec "{swift}" "$@"'.format(
            host_libs = host_libs,
            linker = repository_ctx.path("linker"),
            swift = swift,
        )
    repository_ctx.file("swift", "#!/bin/sh\n{}\n".format(script), executable = True)
    repository_ctx.file("BUILD.bazel", 'exports_files(["swift"], visibility = ["//visibility:public"])\n')

swift_host_repo = repository_rule(
    implementation = _swift_host_impl,
    doc = "A `swift` command that runs the managed toolchain on Linux and the Xcode one on macOS.",
)

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

_SEVEN_ZIP = "_7zip"

def _swift_windows_impl(repository_ctx):
    cell = repository_ctx.attr.cell
    version = repository_ctx.attr.version
    repository_ctx.file("BUILD.bazel", _SWIFT_WINDOWS_BUILD)
    repository_ctx.file(".swift-version", version + "\n")
    if host_cell(repository_ctx) != cell:
        for path in ["toolchain/usr/bin/swiftc.exe", "toolchain/usr/bin/swift-symbolgraph-extract.exe"]:
            repository_ctx.file(path, "")
        return repository_ctx.repo_metadata(reproducible = True)
    repository_ctx.download(
        url = repository_ctx.attr.installer_urls,
        output = "swift.exe",
        sha256 = repository_ctx.attr.installer_sha256,
        canonical_id = "swift-windows-" + version + "-" + cell,
    )
    repository_ctx.download(
        url = repository_ctx.attr.seven_zip_urls,
        output = _SEVEN_ZIP + "/installer.exe",
        sha256 = repository_ctx.attr.seven_zip_sha256,
        canonical_id = "swift-windows-7zip-" + cell,
    )
    seven_zip_extract(
        repository_ctx,
        seven_zip_fetch(repository_ctx, _SEVEN_ZIP + "/reduced"),
        repository_ctx.path(_SEVEN_ZIP + "/installer.exe"),
        _SEVEN_ZIP + "/full",
        args = ["-t7z", "-i!7z.exe", "-i!7z.dll"],
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
            version,
            "-SevenZip",
            repository_ctx.path(_SEVEN_ZIP + "/full/7z.exe"),
        ],
        timeout = 3600,
    )
    if result.return_code != 0:
        fail("extracting the Swift Windows installer failed ({}):\n{}\n{}".format(result.return_code, result.stdout, result.stderr))
    repository_ctx.delete("swift.exe")
    repository_ctx.delete(_SEVEN_ZIP)
    return repository_ctx.repo_metadata(reproducible = True)

swift_windows_repo = repository_rule(
    implementation = _swift_windows_impl,
    attrs = {
        "cell": attr.string(mandatory = True, values = ["windows_arm64", "windows_x86_64"]),
        "installer_sha256": attr.string(mandatory = True),
        "installer_urls": attr.string_list(mandatory = True),
        "seven_zip_sha256": attr.string(mandatory = True),
        "seven_zip_urls": attr.string_list(mandatory = True),
        "version": attr.string(mandatory = True),
    },
    doc = "The sha256-pinned swift.org Windows toolchain, unpacked on a Windows host with the pinned 7-Zip and stubbed elsewhere.",
)
