"""Repository rules for the Swift Linux and Windows toolchains."""

load("//core:host.bzl", "host_cell")
load("//core:seven_zip.bzl", "seven_zip_extract", "seven_zip_fetch")

_SWIFT_LINUX_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files(
    [".swift-version"] + glob(
        [
            "usr/bin/*",
            "usr/lib/clang/*/lib/linux/*.a",
            "usr/lib/swift/linux/*.so*",
        ],
    ),
)

filegroup(
    name = "files",
    srcs = glob(
        include = ["**/*"],
        exclude = ["BUILD.bazel"],
    ),
)

filegroup(
    name = "swift_tool_inputs",
    srcs = glob(
        [
            "usr/lib/swift/**",
            "usr/bin/swift",
            "usr/bin/swift-frontend",
        ],
    ),
)

filegroup(
    name = "linux_dynamic_runtime",
    srcs = glob(["usr/lib/swift/linux/*.so*"]),
)

filegroup(
    name = "linux_static_runtime",
    srcs = glob([
        "usr/lib/swift/linux/*.a",
        "usr/lib/swift_static/linux/*.a",
        "usr/lib/swift_static/linux/*.lnk",
        "usr/lib/swift_static/linux/*/*.o",
    ]),
)
"""

def _swift_linux_impl(repository_ctx):
    version = repository_ctx.attr.version
    prefix = "swift-{}-RELEASE-ubuntu22.04{}".format(version, "-aarch64" if repository_ctx.attr.cell == "linux_arm64" else "")
    repository_ctx.download_and_extract(
        url = repository_ctx.attr.urls,
        sha256 = repository_ctx.attr.sha256,
        stripPrefix = prefix,
        canonical_id = "swift-linux-" + version + "-" + repository_ctx.attr.cell,
    )
    if not repository_ctx.path("usr/bin/llvm-strip").exists:
        repository_ctx.symlink("usr/bin/llvm-objcopy", "usr/bin/llvm-strip")
    host_libs = repository_ctx.path(repository_ctx.attr.sysroot.same_package_label("REPO.bazel")).dirname.get_child("host_libs")
    for lib in host_libs.readdir():
        repository_ctx.symlink(lib, "usr/lib/swift/linux/" + lib.basename)
        repository_ctx.symlink(lib, "usr/lib/" + lib.basename)
    if not repository_ctx.path("usr/bin/ld").exists:
        repository_ctx.symlink("usr/bin/ld.lld", "usr/bin/ld")
    repository_ctx.file(".swift-version", version + "\n")
    repository_ctx.file("BUILD.bazel", _SWIFT_LINUX_BUILD)
    return repository_ctx.repo_metadata(reproducible = True)

swift_linux_repo = repository_rule(
    implementation = _swift_linux_impl,
    attrs = {
        "cell": attr.string(mandatory = True, values = ["linux_arm64", "linux_x86_64"]),
        "sha256": attr.string(mandatory = True),
        "sysroot": attr.label(mandatory = True, doc = "The files of the Ubuntu sysroot whose `host_libs` the tools load."),
        "urls": attr.string_list(mandatory = True),
        "version": attr.string(mandatory = True),
    },
    doc = "The sha256-pinned swift.org Ubuntu 22.04 toolchain, with the host libraries of its sysroot.",
)

_SWIFT_WINDOWS_BUILD = """load("@rules_swift//swift/toolchains:swift_tools.bzl", "swift_tools")

package(default_visibility = ["//visibility:public"])

exports_files([
    ".swift-version",
    "toolchain/usr/bin/swiftc.exe",
])

alias(
    name = "swiftc",
    actual = "toolchain/usr/bin/swiftc.exe",
)

filegroup(
    name = "toolchain_files",
    srcs = glob(
        [
            "toolchain/**",
            "runtime/**",
            "platform/**",
        ],
        exclude = ["**/*.pdb"],
        allow_empty = True,
    ),
)

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

_NO_HOST_TOOLS = {
    "PATH": None,
    "Path": None,
    "PSModulePath": None,
    "SystemRoot": None,
    "SYSTEMROOT": None,
    "windir": None,
    "WINDIR": None,
    "POWERSHELL_TELEMETRY_OPTOUT": "1",
    "POWERSHELL_UPDATECHECK": "Off",
}

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
            "-Cell",
            cell,
        ],
        environment = _NO_HOST_TOOLS,
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
