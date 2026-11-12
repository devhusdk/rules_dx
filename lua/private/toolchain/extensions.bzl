"""The lua extension: the dx_lua_toolchains hub that registers the Lua toolchain."""

load("//core:toolchain.bzl", "toolchain_target")
load(":toolchain.bzl", "LUA_TOOLCHAIN_TYPE")

def lua_hub():
    """Returns the BUILD file of the dx_lua_toolchains hub."""
    return toolchain_target(
        name = "lua",
        toolchain = str(Label(":lua")),
        toolchain_type = str(LUA_TOOLCHAIN_TYPE),
        visibility = ["//visibility:public"],
    )

def _hub_impl(rctx):
    rctx.file("BUILD.bazel", rctx.attr.build)
    return rctx.repo_metadata(reproducible = True)

_hub = repository_rule(
    implementation = _hub_impl,
    doc = "Writes the BUILD file of the Lua toolchain hub.",
    attrs = {"build": attr.string(mandatory = True, doc = "BUILD file content of the hub.")},
)

def _lua_impl(mctx):
    _hub(name = "dx_lua_toolchains", build = lua_hub())
    return mctx.extension_metadata(reproducible = True)

lua = module_extension(
    implementation = _lua_impl,
    doc = "Declares the dx_lua_toolchains hub.",
)
