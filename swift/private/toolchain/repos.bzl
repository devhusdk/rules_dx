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
            "usr/bin/swift-driver",
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

_HOST_LIBS = [
    "libbrotlicommon.so.1",
    "libbrotlidec.so.1",
    "libbsd.so.0",
    "libcom_err.so.2",
    "libcrypto.so.3",
    "libcurl.so.4",
    "libedit.so.2",
    "libffi.so.8",
    "libgcc_s.so.1",
    "libgmp.so.10",
    "libgnutls.so.30",
    "libgssapi_krb5.so.2",
    "libhogweed.so.6",
    "libicudata.so.70",
    "libicuuc.so.70",
    "libidn2.so.0",
    "libk5crypto.so.3",
    "libkeyutils.so.1",
    "libkrb5.so.3",
    "libkrb5support.so.0",
    "liblber-2.5.so.0",
    "libldap-2.5.so.0",
    "liblzma.so.5",
    "libmd.so.0",
    "libncurses.so.6",
    "libnettle.so.8",
    "libnghttp2.so.14",
    "libp11-kit.so.0",
    "libpsl.so.5",
    "librtmp.so.1",
    "libsasl2.so.2",
    "libsqlite3.so.0",
    "libssh.so.4",
    "libssl.so.3",
    "libstdc++.so.6",
    "libtasn1.so.6",
    "libtinfo.so.6",
    "libunistring.so.2",
    "libuuid.so.1",
    "libxml2.so.2",
    "libz.so.1",
    "libzstd.so.1",
]

_TOOL_ONLY_LIBS = [
    "libgcc_s.so.1",
    "libstdc++.so.6",
]

_LINK_LIBS = {
    "libcurl.so": "usr/lib/{triple}/libcurl.so.4",
    "libgcc_eh.a": "usr/lib/gcc/{triple}/11/libgcc_eh.a",
    "libstdc++.a": "usr/lib/gcc/{triple}/11/libstdc++.a",
    "libxml2.so": "usr/lib/{triple}/libxml2.so.2",
}

_SWIFT_LIBS_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files({files})

filegroup(
    name = "host_libs",
    srcs = {host_libs},
)

filegroup(
    name = "runtime_libs",
    testonly = True,
    srcs = {runtime_libs},
)
"""

def _swift_libs_linux_impl(repository_ctx):
    triple = repository_ctx.attr.triple
    sysroot = repository_ctx.path(repository_ctx.attr.sysroot.same_package_label("REPO.bazel")).dirname
    patchelf = repository_ctx.attr._patchelf
    for lib in _HOST_LIBS:
        candidates = [sysroot.get_child(d).get_child(lib) for d in ["lib/" + triple, "usr/lib/" + triple]]
        found = [c for c in candidates if c.exists]
        if not found:
            fail("{} is in neither lib/{} nor usr/lib/{} of the sysroot".format(lib, triple, triple))
        out = "host_libs/" + lib
        repository_ctx.file(out, "")
        result = repository_ctx.execute([patchelf, "--set-rpath", "$ORIGIN", "--output", repository_ctx.path(out), found[0].realpath])
        if result.return_code != 0:
            fail("patchelf {} failed ({}):\n{}\n{}".format(lib, result.return_code, result.stdout, result.stderr))
    for name, path in _LINK_LIBS.items():
        repository_ctx.symlink(sysroot.get_child(path.format(triple = triple)).realpath, "link_libs/" + name)
    host_libs = ["host_libs/" + lib for lib in _HOST_LIBS]
    repository_ctx.file("BUILD.bazel", _SWIFT_LIBS_BUILD.format(
        files = repr(host_libs + ["link_libs/" + name for name in sorted(_LINK_LIBS)]),
        host_libs = repr(host_libs),
        runtime_libs = repr(["host_libs/" + lib for lib in _HOST_LIBS if lib not in _TOOL_ONLY_LIBS]),
    ))
    return repository_ctx.repo_metadata(reproducible = True)

swift_libs_linux_repo = repository_rule(
    implementation = _swift_libs_linux_impl,
    attrs = {
        "sysroot": attr.label(mandatory = True, doc = "The files of the Ubuntu sysroot the libraries come from."),
        "triple": attr.string(mandatory = True, values = ["aarch64-linux-gnu", "x86_64-linux-gnu"]),
        "_patchelf": attr.label(default = Label("@dx_patchelf//:bin/patchelf")),
    },
    doc = "The sysroot libraries the Swift tools load, with their RUNPATH set to their own directory, and the libraries Swift binaries link.",
)

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
    host_libs = repository_ctx.path(repository_ctx.attr.libs.same_package_label("BUILD.bazel")).dirname.get_child("host_libs")
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
        "libs": attr.label(mandatory = True, doc = "A file of the libraries repository whose `host_libs` the tools load."),
        "sha256": attr.string(mandatory = True),
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
            "platform/Developer/Library/XCTest-*/usr/bin64/*.dll",
            "platform/Developer/Library/Testing-*/usr/bin64/*.dll",
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
