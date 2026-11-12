"""Lua rules for dx: the core Lua rules plus dx quality source ownership."""

load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:fuzz_bench.bzl", "merge_tags")
load("//core:wrapper.bzl", "dx_executable_forward_rule", "dx_language_spec", "dx_lcov_merger_attr", "dx_quality_sources", "dx_upstream", "dx_wrap_binary", "dx_wrap_test")
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
load("//core/quality:sources.bzl", "QualitySourcesInfo")

LuaInfo = _LuaInfo
LUA_ENTRY_FORMAT = _LUA_ENTRY_FORMAT
LUA_ENTRY_SUFFIX = _LUA_ENTRY_SUFFIX

def _quality(ctx, files):
    return dx_quality_sources(files, LUA_SOURCE_SPECS, str(ctx.label))

def _library_impl(ctx):
    return lua_library_impl(ctx) + [_quality(ctx, ctx.files.srcs)]

_lua_library = rule(
    implementation = _library_impl,
    doc = "Lua modules other Lua targets can require.",
    attrs = LUA_LIBRARY_ATTRS,
    provides = [LuaInfo, QualitySourcesInfo],
    toolchains = LUA_TOOLCHAINS,
)

_lua_binary = rule(
    implementation = lua_executable_impl,
    doc = "A Lua program that bazel run starts through the shared launcher.",
    attrs = LUA_EXECUTABLE_ATTRS,
    executable = True,
    toolchains = LUA_TOOLCHAINS,
)

_lua_test = rule(
    implementation = lua_executable_impl,
    doc = "A Lua test; it passes when main returns or calls os.exit(0).",
    attrs = LUA_TEST_ATTRS,
    test = True,
    toolchains = LUA_TOOLCHAINS,
)

def _entry_runfiles(ctx):
    upstream = dx_upstream(ctx)
    entries = [f for f in upstream[DefaultInfo].files.to_list() if f.basename.endswith(LUA_ENTRY_SUFFIX)]
    link = ctx.actions.declare_file(ctx.label.name + LUA_ENTRY_SUFFIX)
    ctx.actions.symlink(output = link, target_file = entries[0])
    return ctx.runfiles(files = [link])

_LUA = dx_language_spec(what = "lua_binary", quality_specs = LUA_SOURCE_SPECS, allow_files = [".lua"])

_lua_binary_forward = dx_executable_forward_rule(
    _LUA,
    kind = "executable",
    coverage_runfiles = _entry_runfiles,
    runtime = "mandatory",
)

_lua_forward_test = dx_executable_forward_rule(
    _LUA,
    kind = "test",
    what = "lua_test",
    coverage_runfiles = _entry_runfiles,
    extra_attrs = dx_lcov_merger_attr(),
    runtime = "mandatory",
)

def lua_library(name, srcs = None, visibility = None, **kwargs):
    """Lua modules other Lua targets can require."""
    _lua_library(name = name, srcs = srcs or [], visibility = visibility, **kwargs)

def lua_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """A Lua program that bazel run starts through the shared launcher."""
    dx_wrap_binary(name, _lua_binary, _lua_binary_forward, srcs or [], visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def lua_test(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """A Lua test; it passes when main returns or calls os.exit(0)."""
    dx_wrap_test(name, _lua_test, _lua_forward_test, srcs or [], visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def lua_bench(name, *, srcs = None, main = None, deps = None, tags = None, **kwargs):
    """A lua_test run by dx bench: main calls require("dx_bench").run({{name, fn}, ...}), which times each function with os.clock and writes dx-bench.json."""
    main = dx_infer_main("lua_bench", name, main, srcs, dx_main_candidates(name, ".lua"))
    lua_test(
        name = name,
        srcs = srcs,
        main = main,
        deps = (deps or []) + [Label("//lua/private:bench")],
        tags = merge_tags(tags, "dx-bench"),
        **kwargs
    )
