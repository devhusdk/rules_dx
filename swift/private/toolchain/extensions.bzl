"""The swift extension: the Swift archives, the Windows installers and the dx_swift_toolchains hub."""

load("@dx_pins_data//:pins.bzl", "PINS")
load("//core:toolchain.bzl", "toolchain_target")
load(":repos.bzl", "swift_libs_linux_repo", "swift_linux_repo", "swift_windows_repo")
load(":toolchain.bzl", "SWIFT_LINUX_CPUS")

_TOOLCHAIN_TYPE = "@rules_swift//toolchains:toolchain_type"
_WINDOWS_CPUS = ["aarch64", "x86_64"]

def _platform(os, cpu):
    return ["@platforms//os:" + os, "@platforms//cpu:" + cpu]

def swift_hub():
    """Returns the BUILD file of the dx_swift_toolchains hub."""
    linux = [
        toolchain_target(
            name = "toolchain_%s_%s" % (exec_cpu, target_cpu),
            exec_compatible_with = _platform("linux", exec_cpu),
            target_compatible_with = _platform("linux", target_cpu),
            toolchain = str(Label(":swift_toolchain_%s_%s" % (exec_cpu, target_cpu))),
            toolchain_type = _TOOLCHAIN_TYPE,
            visibility = ["//visibility:public"],
        )
        for exec_cpu in SWIFT_LINUX_CPUS
        for target_cpu in SWIFT_LINUX_CPUS
    ]
    windows = [
        toolchain_target(
            name = "toolchain_windows_" + cpu,
            exec_compatible_with = _platform("windows", cpu),
            target_compatible_with = _platform("windows", cpu),
            toolchain = str(Label(":swift_toolchain_windows_" + cpu)),
            toolchain_type = _TOOLCHAIN_TYPE,
            visibility = ["//visibility:public"],
        )
        for cpu in _WINDOWS_CPUS
    ]
    return "\n".join(sorted(linux + windows))

def _hub_impl(rctx):
    rctx.file("BUILD.bazel", rctx.attr.build)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Writes the BUILD file of the Swift toolchain hub.",
    attrs = {"build": attr.string(mandatory = True, doc = "BUILD file content of the hub.")},
)

def _swift_impl(module_ctx):
    pins = PINS["dx_swift_format_macos_arm64"]
    for cpu, cell in [("x86_64", "linux_x86_64"), ("aarch64", "linux_arm64")]:
        linux = pins["swift_linux"]
        swift_libs_linux_repo(
            name = "dx_swift_libs_linux_" + cpu,
            sysroot = Label("@dx_swift_sysroot_linux_%s//:files" % cpu),
            triple = cpu + "-linux-gnu",
        )
        swift_linux_repo(
            name = "dx_swift_linux_" + cpu,
            cell = cell,
            libs = "@dx_swift_libs_linux_%s//:BUILD.bazel" % cpu,
            sha256 = linux["cells"][cell]["sha256"],
            urls = linux["cells"][cell]["urls"],
            version = linux["version"],
        )
    for cpu, cell in [("x86_64", "windows_x86_64"), ("aarch64", "windows_arm64")]:
        installer = pins["swift_windows"]["cells"][cell]
        seven_zip = pins["seven_zip_windows"]["cells"][cell]
        swift_windows_repo(
            name = "dx_swift_windows_" + cpu,
            cell = cell,
            installer_sha256 = installer["sha256"],
            installer_urls = installer["urls"],
            seven_zip_sha256 = seven_zip["sha256"],
            seven_zip_urls = seven_zip["urls"],
            version = pins["swift_windows"]["version"],
        )
    _hub(name = "dx_swift_toolchains", build = swift_hub())
    return module_ctx.extension_metadata(reproducible = True)

swift = module_extension(
    implementation = _swift_impl,
    doc = "Declares the Swift archives, the Windows installers and the dx_swift_toolchains hub.",
)
