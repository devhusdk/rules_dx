"""Module extension for swift/toolchain."""

load(":repos.bzl", "swift_host_repo", "swift_windows_repo")


def _swift_host_ext_impl(module_ctx):
    swift_host_repo(name = "dx_swift_host")
    return module_ctx.extension_metadata(reproducible = True)


swift_host = module_extension(implementation = _swift_host_ext_impl)


def _swift_windows_ext_impl(module_ctx):
    swift_windows_repo(name = "dx_swift_windows_x86_64", cpu = "x86_64")
    swift_windows_repo(name = "dx_swift_windows_aarch64", cpu = "aarch64")
    return module_ctx.extension_metadata(reproducible = True)


swift_windows = module_extension(implementation = _swift_windows_ext_impl)
