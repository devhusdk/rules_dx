"""The dx_julia_toolchains hub: the toolchain that resolves Julia on Linux x86_64 to the source-built runtime."""

load("//core:repo.bzl", "hub_repo")

HUB_NAME = "dx_julia_toolchains"

def julia_hub():
    """Returns the BUILD file of the dx_julia_toolchains hub."""
    return "\n".join([
        "load({}, \"julia_source_toolchain_target\")".format(repr(str(Label("//julia/private/runtime:defs.bzl")))),
        "",
        "julia_source_toolchain_target(name = \"source_toolchain\")",
        "",
    ])

def _dx_julia_toolchains_impl(module_ctx):
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": julia_hub()})
    return module_ctx.extension_metadata(reproducible = True)

dx_julia_toolchains = module_extension(
    implementation = _dx_julia_toolchains_impl,
    doc = "Declares the `dx_julia_toolchains` hub that rules_dx registers for consumers.",
)
