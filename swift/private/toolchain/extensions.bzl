"""Module extension for swift/private/toolchain."""

load("@dx_pins_data//:pins.bzl", "PINS")
load(":repos.bzl", "swift_linux_repo", "swift_windows_repo")


def _swift_windows_ext_impl(module_ctx):
    pins = PINS["dx_swift_format_macos_arm64"]
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
    return module_ctx.extension_metadata(reproducible = True)


swift_windows = module_extension(implementation = _swift_windows_ext_impl)

def _swift_linux_ext_impl(module_ctx):
    pins = PINS["dx_swift_format_macos_arm64"]["swift_linux"]
    for cpu, cell in [("x86_64", "linux_x86_64"), ("aarch64", "linux_arm64")]:
        swift_linux_repo(
            name = "dx_swift_linux_" + cpu,
            cell = cell,
            sha256 = pins["cells"][cell]["sha256"],
            sysroot = Label("@dx_swift_sysroot_linux_%s//:files" % cpu),
            urls = pins["cells"][cell]["urls"],
            version = pins["version"],
        )
    return module_ctx.extension_metadata(reproducible = True)

swift_linux = module_extension(implementation = _swift_linux_ext_impl)
