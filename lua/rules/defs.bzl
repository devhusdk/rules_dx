"""Lua rules for dx: the core Lua rules plus dx quality source ownership."""

load("//libs/starlark:wrapper.bzl", "dx_quality_sources", "dx_quality_srcs")
load(
    "//lua/private:lua.bzl",
    "LUA_EXECUTABLE_ATTRS",
    "LUA_LIBRARY_ATTRS",
    "LUA_SOURCE_SPECS",
    "LUA_TEST_ATTRS",
    "LUA_TOOLCHAINS",
    "lua_executable_impl",
    "lua_library_impl",
)
load("//lua/private:providers.bzl", _LUA_ENTRY_FORMAT = "LUA_ENTRY_FORMAT", _LUA_ENTRY_SUFFIX = "LUA_ENTRY_SUFFIX", _LuaInfo = "LuaInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

LuaInfo = _LuaInfo
LUA_ENTRY_FORMAT = _LUA_ENTRY_FORMAT
LUA_ENTRY_SUFFIX = _LUA_ENTRY_SUFFIX

def _quality(ctx, files):
    return dx_quality_sources(files, LUA_SOURCE_SPECS, str(ctx.label))

def _library_impl(ctx):
    return lua_library_impl(ctx) + [_quality(ctx, ctx.files.srcs)]

def _executable_impl(ctx):
    return lua_executable_impl(ctx) + [_quality(ctx, dx_quality_srcs(ctx.file.main, ctx.files.srcs))]

_lua_library = rule(
    implementation = _library_impl,
    doc = "Lua modules other Lua targets can require.",
    attrs = LUA_LIBRARY_ATTRS,
    provides = [LuaInfo, QualitySourcesInfo],
    toolchains = LUA_TOOLCHAINS,
)

_lua_binary = rule(
    implementation = _executable_impl,
    doc = "A Lua program that bazel run starts through the shared launcher.",
    attrs = LUA_EXECUTABLE_ATTRS,
    executable = True,
    toolchains = LUA_TOOLCHAINS,
)

_lua_test = rule(
    implementation = _executable_impl,
    doc = "A Lua test; it passes when main returns or calls os.exit(0).",
    attrs = LUA_TEST_ATTRS,
    test = True,
    toolchains = LUA_TOOLCHAINS,
)

def lua_library(name, srcs = None, visibility = None, **kwargs):
    """Lua modules other Lua targets can require."""
    _lua_library(name = name, srcs = srcs or [], visibility = visibility, **kwargs)

def lua_binary(name, srcs = None, visibility = None, **kwargs):
    """A Lua program that bazel run starts through the shared launcher."""
    _lua_binary(name = name, srcs = srcs or [], visibility = visibility, **kwargs)

def lua_test(name, srcs = None, visibility = None, **kwargs):
    """A Lua test; it passes when main returns or calls os.exit(0)."""
    kwargs.setdefault("size", "small")
    _lua_test(name = name, srcs = srcs or [], visibility = visibility, **kwargs)
