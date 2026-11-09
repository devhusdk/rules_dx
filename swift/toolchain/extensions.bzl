"""Module extension for swift/toolchain."""

load(":repos.bzl", "swift_format_macos_repo", "swift_host_repo")


def _swift_host_ext_impl(module_ctx):
    swift_host_repo(name = "dx_swift_host")
    return module_ctx.extension_metadata(reproducible = True)


swift_host = module_extension(implementation = _swift_host_ext_impl)


def _swift_format_macos_ext_impl(module_ctx):
    swift_format_macos_repo(name = "dx_swift_format_macos_arm64")
    return module_ctx.extension_metadata(reproducible = True)


swift_format_macos = module_extension(implementation = _swift_format_macos_ext_impl)
