"""Module extension for erlang/private/toolchain."""

load("//core:repo.bzl", "hub_repo")
load("//elixir/private:hex.bzl", "hex_package")
load(":pins.bzl", "CELLS", "HEX_SOURCES")
load(":repos.bzl", "HEX_BUILD", "HUB_NAME", "cell_repository_name", "hub_build", "otp_repo")

HEX_DEPS = {"dx_erlperf": ["@dx_erlang_argparse//:erlang_app"]}

def _erlang_impl(mctx):
    for cell in sorted(CELLS.keys()):
        otp_repo(name = cell_repository_name(cell), cell = cell)
    hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build()})
    for name, hex in sorted(HEX_SOURCES.items()):
        hex_package(
            name = name,
            build_file_content = HEX_BUILD.format(app = hex.package, version = hex.version, deps = repr(HEX_DEPS.get(name, []))),
            package = hex.package,
            sha256 = hex.sha256,
            version = hex.version,
        )
    return mctx.extension_metadata(reproducible = True)

erlang = module_extension(
    implementation = _erlang_impl,
    doc = "Creates the pinned OTP trees, the toolchain hub and the erlperf Hex packages.",
)
