"""Thin dx wrappers over the core Lua rules."""

load("//core:entry.bzl", "dx_entry_quality_srcs", "dx_infer_main", "dx_main_candidates")
load("//core:fuzz_bench.bzl", "merge_tags")
load("//core:wrapper.bzl", "dx_executable_forward_rule", "dx_forward_spec", "dx_language_spec", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_private_dep_rule", "dx_upstream", "dx_upstream_tags", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo")
load("//lua/private:lua.bzl", "write_entry", _core_lua_binary = "lua_binary", _core_lua_library = "lua_library", _core_lua_test = "lua_test")
load("//lua/private:providers.bzl", "LUA_COVERAGE_MODULE", _LUA_ENTRY_FORMAT = "LUA_ENTRY_FORMAT", _LUA_ENTRY_SUFFIX = "LUA_ENTRY_SUFFIX", _LuaEntryInfo = "LuaEntryInfo", _LuaInfo = "LuaInfo")

LuaInfo = _LuaInfo
LuaEntryInfo = _LuaEntryInfo
LUA_ENTRY_FORMAT = _LUA_ENTRY_FORMAT
LUA_ENTRY_SUFFIX = _LUA_ENTRY_SUFFIX

LUA_SOURCE_SPECS = [("lua", ["lua"])]

def _entry_runfiles(ctx):
    upstream = dx_upstream(ctx)
    entries = [f for f in upstream[DefaultInfo].files.to_list() if f.basename.endswith(LUA_ENTRY_SUFFIX)]
    link = ctx.actions.declare_file(ctx.label.name + LUA_ENTRY_SUFFIX)
    ctx.actions.symlink(output = link, target_file = entries[0])
    return ctx.runfiles(files = [link])

def _test_entry_runfiles(ctx):
    if not ctx.configuration.coverage_enabled:
        return _entry_runfiles(ctx)
    upstream = dx_upstream(ctx)[_LuaEntryInfo]
    coverage = ctx.attr._lua_coverage[_LuaInfo]
    modules = depset(transitive = [upstream.modules, coverage.modules])
    if LUA_COVERAGE_MODULE not in [name for name, _ in coverage.modules.to_list()]:
        fail("%s: %s provides no %s module" % (ctx.label, ctx.attr._lua_coverage.label, LUA_COVERAGE_MODULE))
    entry = ctx.actions.declare_file(ctx.label.name + LUA_ENTRY_SUFFIX)
    write_entry(ctx, entry, upstream.main, upstream.source_repo, modules)
    return ctx.runfiles(files = [entry]).merge(coverage.runfiles)

_LUA = dx_language_spec(what = "lua_*", quality_specs = LUA_SOURCE_SPECS, allow_files = [".lua"])

_lua_library_forward = dx_library_forward_rule(
    _LUA,
    dx_forward_spec(
        provides = [_LuaInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
        required_providers = [(_LuaInfo, "LuaInfo")],
        upstream_providers = [[_LuaInfo]],
        runtime = "mandatory",
    ),
)

_lua_binary_forward = dx_executable_forward_rule(
    _LUA,
    "executable",
    dx_forward_spec(
        coverage_runfiles = _entry_runfiles,
        runtime = "mandatory",
    ),
)

_lua_forward_test = dx_executable_forward_rule(
    _LUA,
    "test",
    dx_forward_spec(
        coverage_runfiles = _test_entry_runfiles,
        extra_attrs = dx_lcov_merger_attr() | {
            "_lua_coverage": attr.label(
                default = Label("//lua/private/coverage:libs"),
                providers = [_LuaInfo],
                doc = "luacov, its LCOV reporter and the dx_coverage hook, empty outside bazel coverage.",
            ),
        },
        runtime = "mandatory",
    ),
)

def lua_library(name, srcs = None, visibility = None, **kwargs):
    """Lua modules other Lua targets can require."""
    dx_wrap(name, _core_lua_library, _lua_library_forward, srcs or [], visibility = visibility, **kwargs)

def lua_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """A Lua program that bazel run starts through the shared launcher."""
    dx_wrap_binary(name, _core_lua_binary, _lua_binary_forward, srcs or [], visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

def lua_test(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """A Lua test; it passes when main returns or calls os.exit(0)."""
    dx_wrap_test(name, _core_lua_test, _lua_forward_test, srcs or [], visibility = visibility, upstream_kwargs = dict(kwargs, main = main), quality_srcs = dx_entry_quality_srcs(main, srcs), **kwargs)

_lua_bench_lib = dx_private_dep_rule(
    Label("//lua/private:bench"),
    [_LuaInfo, DefaultInfo, InstrumentedFilesInfo, OutputGroupInfo, QualitySourcesInfo],
    "Provides the dx_bench module lua_bench requires.",
)

def lua_bench(name, *, srcs = None, main = None, deps = None, tags = None, **kwargs):
    """A lua_test run by dx bench: main calls require("dx_bench").run({{name, fn}, ...}), which times each function with os.clock and writes dx-bench.json."""
    main = dx_infer_main("lua_bench", name, main, srcs, dx_main_candidates(name, ".lua"))
    _lua_bench_lib(
        name = name + "_dx_bench",
        testonly = True,
        tags = dx_upstream_tags(tags),
        visibility = ["//visibility:private"],
    )
    lua_test(
        name = name,
        srcs = srcs,
        main = main,
        deps = (deps or []) + [":" + name + "_dx_bench"],
        tags = merge_tags(tags, "dx-bench"),
        **kwargs
    )
