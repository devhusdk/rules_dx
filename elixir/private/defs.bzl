"""Thin Elixir wrappers over rules_elixir."""

load("@rules_elixir//:elixir_app.bzl", _elixir_app = "elixir_app")
load("@rules_elixir//:ex_unit_test.bzl", _ex_unit_test = "ex_unit_test")
load("@rules_erlang//:erlang_app_info.bzl", "flat_deps", _ErlangAppInfo = "ErlangAppInfo")
load("//erlang/private:beam.bzl", "beam_info")
load("//core:rlocation.bzl", "runfiles_path")
load("//erlang/private:defs.bzl", _escript_archive = "erlang_escript_archive")
load("//erlang/private:providers.bzl", "BeamInfo", "DialyzerPltInfo", "ErlangNifInfo")
load("//erlang/private/toolchain:cells.bzl", "otp_compatible_with")
load("//erlang/private/toolchain:pins.bzl", "OTP_DIALYZER_UNPORTED_CELLS")
load("//core:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//core:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_upstream_tags", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo")
load(":kwargs.bzl", "elixir_app_kwargs", "elixir_cover_opts", "elixir_kwargs", _COVER_LOAD = "COVER_LOAD", _ELIXIR_CELLS = "ELIXIR_CELLS", _ELIXIR_COMPATIBLE_WITH = "ELIXIR_COMPATIBLE_WITH")

COVER_LOAD = _COVER_LOAD
ELIXIR_CELLS = _ELIXIR_CELLS
ELIXIR_COMPATIBLE_WITH = _ELIXIR_COMPATIBLE_WITH

ErlangAppInfo = _ErlangAppInfo

ELIXIR_DIALYZER_COMPATIBLE_WITH = otp_compatible_with([cell for cell in ELIXIR_CELLS if cell not in OTP_DIALYZER_UNPORTED_CELLS])

ELIXIR_SOURCE_GLOBS = ["lib/**/*.ex"]

# Re-exported unchanged: BUILD files need it to build escripts.
escript_archive = _escript_archive

def escript_headers(module):
    """Returns escript_archive headers that start one Elixir module in a UTF-8 VM."""
    return ["shebang", '{emu_args, "+fnu -kernel standard_io_encoding unicode -escript main Elixir.' + module + '"}']

_SPECS = [("elixir", ["ex", "exs"])]

_EXTS = [".ex", ".exs"]

def _beam_info(ctx, upstream):
    nifs = [nif[ErlangNifInfo] for nif in ctx.attr.nifs]
    plts = [dep[DialyzerPltInfo].plt for dep in ctx.attr._dialyzer_plts if DialyzerPltInfo in dep]
    code = [f for dep in ctx.attr._dialyzer_code for f in dep[DefaultInfo].files.to_list()]
    return [ErlangNifInfo(
        libraries = depset(transitive = [nif.libraries for nif in nifs]),
        objects_lists = depset(transitive = [nif.objects_lists for nif in nifs]),
    ), beam_info(upstream, plts, code)]

_elixir_app_forward = dx_library_forward_rule(
    extra_providers = _beam_info,
    extra_attrs = {
        "nifs": attr.label_list(
            doc = "erlang_nif targets this app loads from its priv directory.",
            providers = [ErlangNifInfo],
        ),
        "_dialyzer_code": attr.label_list(
            default = ["//elixir/private/tools:dialyzer_code_if_ported"],
            doc = "The Elixir compiler beams dialyzer loads to read the debug info of Elixir beams.",
        ),
        "_dialyzer_plts": attr.label_list(
            default = ["//elixir/private/tools:elixir_plt_if_ported"],
            doc = "The Elixir standard-library PLT dialyzer checks Elixir beams against; empty where dialyzer is not ported.",
        ),
    },
    provides = [_ErlangAppInfo, BeamInfo, ErlangNifInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_ErlangAppInfo, "ErlangAppInfo")],
    quality_specs = _SPECS,
    what = "elixir_app",
    allow_files = _EXTS,
    upstream_providers = [[_ErlangAppInfo]],
    runtime = "besteffort",
)

def cover_sources(apps):
    """Returns the app names and `.ex` paths of the workspace apps a test covers."""
    names = {}
    paths = {}
    for app in apps + flat_deps(apps):
        for f in app[_ErlangAppInfo].srcs:
            if f.path.endswith(".ex") and f.owner != None and f.owner.repo_name == "":
                names[app[_ErlangAppInfo].app_name] = True
                paths[f.short_path] = True
    return sorted(names.keys()), sorted(paths.keys())

def _ex_unit_test_coverage(ctx, upstream):
    """Returns the coverage providers that cover-compile the apps of one test and write LCOV."""
    script = ctx.file._cover_script
    names, paths = cover_sources(ctx.attr.apps)
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    if paths:
        env["DX_ELIXIR_COVER_APPS"] = ",".join(names)
        env["DX_ELIXIR_COVER_SOURCES"] = ",".join(paths)
        env["DX_ELIXIR_COVER_SCRIPT"] = runfiles_path(ctx.workspace_name, script.short_path)
    return struct(
        instrumented = coverage_common.instrumented_files_info(
            ctx,
            source_attributes = ["srcs"],
            dependency_attributes = ["upstream", "apps"],
        ),
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = ctx.runfiles(files = [script]),
    )

_ex_unit_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = [DefaultInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _SPECS,
    what = "ex_unit_test",
    allow_files = _EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr() | {
        "apps": attr.label_list(
            doc = "Elixir app targets whose sources feed test coverage.",
            providers = [_ErlangAppInfo],
        ),
        "_cover_script": attr.label(default = "//elixir/private/coverage:dx_cover.exs", allow_single_file = True),
    },
    runtime = "besteffort",
    coverage_providers = _ex_unit_test_coverage,
)

def app_sources(kwargs, glob = None):
    """Returns the source files one elixir_app owns for its attributes."""
    if kwargs.get("srcs") != None:
        return kwargs["srcs"]
    glob = glob if glob != None else native.glob
    return glob(ELIXIR_SOURCE_GLOBS, allow_empty = True)

def nif_priv(kwargs, nifs, glob = None):
    """Returns the full priv list: globbed priv files, extra_priv, then the NIF libraries."""
    glob = glob if glob != None else native.glob
    extra = list(kwargs.get("extra_priv", []))
    excluded = [e for e in extra if not (e.startswith(":") or e.startswith("//") or e.startswith("@"))]
    return glob(["priv/**/*"], exclude = excluded, allow_empty = True) + extra + list(nifs)

def elixir_app(name, visibility = None, testonly = False, **kwargs):
    """Thin wrapper over the rules_elixir elixir_app macro."""
    kwargs = elixir_kwargs(kwargs)
    tags = kwargs.pop("tags", None)
    nifs = kwargs.pop("nifs", [])
    if nifs:
        # rules_erlang rejects extra_priv next to srcs, so list priv in full.
        kwargs["priv"] = nif_priv(kwargs, nifs)
        kwargs.pop("extra_priv", None)
    owned = app_sources(kwargs)
    _elixir_app(name = name + "_upstream", testonly = testonly, tags = dx_upstream_tags(tags), **elixir_app_kwargs(kwargs, owned))
    forward_kwargs = {"testonly": True} if testonly else {}
    if nifs:
        forward_kwargs["nifs"] = nifs
    if tags != None:
        forward_kwargs["tags"] = tags
    if kwargs.get("target_compatible_with") != None:
        forward_kwargs["target_compatible_with"] = kwargs["target_compatible_with"]
    _elixir_app_forward(
        name = name,
        upstream = ":" + name + "_upstream",
        srcs = owned,
        visibility = visibility,
        **forward_kwargs
    )

def helper_first(srcs):
    """Returns test srcs with test_helper.exs files required first."""
    if type(srcs) != "list":
        return srcs
    helpers = [src for src in srcs if src.split("/")[-1].split(":")[-1] == "test_helper.exs"]
    return helpers + [src for src in srcs if src not in helpers]

def elixir_test(name, *, srcs = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_elixir ex_unit_test rule. srcs is required; test_helper.exs is loaded first and coverage loads through elixir_opts."""
    if srcs == None:
        fail("elixir_test //" + native.package_name() + ":" + name + " needs srcs")
    kwargs = elixir_kwargs(kwargs)
    upstream = dict(kwargs, elixir_opts = elixir_cover_opts(kwargs.get("elixir_opts")))
    apps = {"apps": kwargs["deps"]} if kwargs.get("deps") else None
    dx_wrap_test(name, _ex_unit_test, _ex_unit_forward_test, helper_first(srcs), visibility = visibility, upstream_kwargs = upstream, extra_forward_kwargs = apps, **kwargs)

def elixir_bench(name, *, srcs, tags = None, deps = None, **kwargs):
    """elixir_test run by dx bench: DxBench.run in srcs runs Benchee and writes dx-bench.json."""
    elixir_test(
        name = name,
        srcs = srcs,
        tags = merge_tags(tags, "dx-bench", "manual"),
        deps = (deps or []) + [Label("//elixir/bench:dx_bench")],
        **with_default_size(kwargs)
    )
