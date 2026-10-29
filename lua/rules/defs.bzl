"""Lua rules for dx: the core Lua rules plus dx quality source ownership."""

load("//libs/starlark:wrapper.bzl", "DX_NO_COVERAGE_COLLECTOR_TAG", "dx_quality_sources", "dx_with_tag")
load(
    "//lua/private:lua.bzl",
    "LUA_EXECUTABLE_ATTRS",
    "LUA_LIBRARY_ATTRS",
    "LUA_TEST_ATTRS",
    "LUA_TOOLCHAINS",
    "lua_executable_impl",
    "lua_library_impl",
    _canonical_rlocation = "canonical_rlocation",
    _entry_lines = "entry_lines",
    _lua_executable_name = "lua_executable_name",
    _module_name = "module_name",
)
load("//lua/private:providers.bzl", _LUA_ENTRY_FORMAT = "LUA_ENTRY_FORMAT", _LUA_ENTRY_SUFFIX = "LUA_ENTRY_SUFFIX", _LuaInfo = "LuaInfo")
load("//quality:sources.bzl", "QualitySourcesInfo")

LuaInfo = _LuaInfo
LUA_ENTRY_FORMAT = _LUA_ENTRY_FORMAT
LUA_ENTRY_SUFFIX = _LUA_ENTRY_SUFFIX
module_name = _module_name
entry_lines = _entry_lines
canonical_rlocation = _canonical_rlocation
lua_executable_name = _lua_executable_name

LUA_SOURCE_SPECS = [("lua", ["lua"])]

LUA_DEFERRED_SKIP_TAGS = []

def lua_quality_srcs(main, srcs):
    """Returns the Lua files one target owns: main first, then srcs."""
    owned = [main] if main != None else []
    return owned + [s for s in srcs if s != main]

def lua_deferred_tags(tags):
    """Adds the skip tags that keep the deferred lua class out of quality aspects."""
    out = list(tags or [])
    for tag in LUA_DEFERRED_SKIP_TAGS:
        if tag not in out:
            out.append(tag)
    return out

def _quality(ctx, files):
    return dx_quality_sources(files, LUA_SOURCE_SPECS, str(ctx.label))

def _library_impl(ctx):
    return lua_library_impl(ctx) + [_quality(ctx, ctx.files.srcs)]

def _executable_impl(ctx):
    return lua_executable_impl(ctx) + [_quality(ctx, lua_quality_srcs(ctx.file.main, ctx.files.srcs))]

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

def lua_library(name, **kwargs):
    """Lua modules other Lua targets can require."""
    _lua_library(name = name, tags = lua_deferred_tags(kwargs.pop("tags", None)), **kwargs)

def lua_binary(name, **kwargs):
    """A Lua program that bazel run starts through the shared launcher."""
    _lua_binary(name = name, tags = lua_deferred_tags(kwargs.pop("tags", None)), **kwargs)

def lua_test(name, **kwargs):
    """A Lua test; it passes when main returns or calls os.exit(0)."""
    _lua_test(name = name, tags = dx_with_tag(lua_deferred_tags(kwargs.pop("tags", None)), DX_NO_COVERAGE_COLLECTOR_TAG), **kwargs)
