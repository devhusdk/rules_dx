"""Module extension for swift/private/toolchain."""

load("@dx_pins_data//:pins.bzl", "PINS")
load(":repos.bzl", "swift_windows_repo")


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
