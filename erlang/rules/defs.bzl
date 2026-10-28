"""Thin Erlang/OTP wrappers over rules_erlang."""

load("@rules_erlang//:ct.bzl", _ct_suite = "ct_suite")
load("@rules_erlang//:erlang_app.bzl", _DEFAULT_ERLC_OPTS = "DEFAULT_ERLC_OPTS", _DEFAULT_TEST_ERLC_OPTS = "DEFAULT_TEST_ERLC_OPTS", _erlang_app = "erlang_app", _test_erlang_app = "test_erlang_app")
load("@rules_erlang//:erlang_app_info.bzl", "flat_deps", _ErlangAppInfo = "ErlangAppInfo")
load("@rules_erlang//:erlang_bytecode2.bzl", _erlang_bytecode = "erlang_bytecode", _erlc_opts = "erlc_opts")
load("@rules_erlang//:erlang_xrl_yrl.bzl", _erlang_xrl_yrl = "erlang_xrl_yrl")
load("@rules_erlang//:escript.bzl", _escript_archive = "escript_archive")
load("@rules_erlang//:eunit2.bzl", _eunit = "eunit")
load("@rules_erlang//tools:erlang_toolchain.bzl", "erlang_dirs")
load("//erlang/rules:providers.bzl", "BeamInfo")
load("//erlang/toolchain:cells.bzl", "otp_compatible_with")
load("//erlang/toolchain:pins.bzl", "CELLS")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_binary_forward_kwargs", "dx_binary_upstream_kwargs", "dx_executable_forward_rule", "dx_forward_data_attr", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_quality_sources", "dx_setdefaults", "dx_test_forward_kwargs", "dx_test_upstream_kwargs", "dx_upstream_tags", "dx_wrap", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo")

ErlangAppInfo = _ErlangAppInfo

ERLANG_CELLS = sorted(CELLS.keys())

ERLANG_COMPATIBLE_WITH = otp_compatible_with(ERLANG_CELLS)

DEFAULT_ERLC_OPTS = _DEFAULT_ERLC_OPTS

DEFAULT_TEST_ERLC_OPTS = _DEFAULT_TEST_ERLC_OPTS

erlc_opts = _erlc_opts

erlang_xrl_yrl = _erlang_xrl_yrl

ERLANG_SOURCE_GLOBS = ["src/**/*.erl", "src/**/*.hrl", "include/**/*.hrl"]

_SPECS = [("erlang", ["erl", "hrl"])]

_EXTS = [".erl", ".hrl"]

def _beams(info):
    """The .beam files of one ErlangAppInfo; a prebuilt ebin directory is not supported."""
    beams = []
    for f in info.beam:
        if f.is_directory:
            fail("erlang_app " + info.app_name + ": dialyzer needs listed .beam files, not the directory " + f.short_path)
        if f.extension == "beam":
            beams.append(f)
    return beams

def _beam_info(_ctx, upstream):
    info = upstream[_ErlangAppInfo]
    return [BeamInfo(
        app_name = info.app_name,
        owned = depset(_beams(info)),
        deps = depset([b for dep in info.deps for b in _beams(dep[_ErlangAppInfo])]),
        extra_apps = info.extra_apps,
    )]

_erlang_app_forward = dx_library_forward_rule(
    extra_providers = _beam_info,
    provides = [_ErlangAppInfo, BeamInfo, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [(_ErlangAppInfo, "ErlangAppInfo")],
    quality_specs = _SPECS,
    what = "erlang_app",
    allow_files = _EXTS,
    upstream_providers = [[_ErlangAppInfo]],
    runtime = "besteffort",
)

_erlang_bytecode_forward = dx_library_forward_rule(
    provides = [DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    required_providers = [],
    quality_specs = _SPECS,
    what = "erlang_bytecode",
    allow_files = _EXTS,
    upstream_providers = None,
    runtime = "besteffort",
)

_LAUNCHER = """#!/usr/bin/env bash
set -euo pipefail
if [[ -d "$0.runfiles" ]]; then
    runfiles="$0.runfiles"
elif [[ -n "${{RUNFILES_DIR:-}}" ]]; then
    runfiles="$RUNFILES_DIR"
elif [[ "$0" == *.runfiles/* ]]; then
    runfiles="${{0%%.runfiles/*}}.runfiles"
else
    echo "$0: no runfiles tree" >&2
    exit 2
fi
DX_ERLANG_HOME="$(cd "$runfiles/{root}" && pwd)"
{prelude}exec "$DX_ERLANG_HOME/bin/{tool}" {args}"$@"
"""

# dialyzer reads $HOME for its caches even when every PLT is named; hermetic callers may unset it.
_HOME_PRELUDE = 'export HOME="${HOME:-${TMPDIR:-/tmp}}"\n'

def otp_launcher_text(root, tool, args = "", prelude = ""):
    """Returns the shared launcher script that runs `bin/<tool>` of the resolved OTP tree."""
    return _LAUNCHER.format(root = root, tool = tool, args = args, prelude = prelude)

def runfiles_path(workspace_name, short_path):
    """Returns one file's path below the runfiles root."""
    if short_path.startswith("../"):
        return short_path[3:]
    return workspace_name + "/" + short_path

def otp_tool_binary_impl(ctx):
    """Implements `otp_tool_binary`: a launcher for one `bin/<tool>` of the resolved OTP tree."""
    (_, _, erlang_runfiles) = erlang_dirs(ctx)
    root = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo.version_file
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, otp_launcher_text(
        root = runfiles_path(ctx.workspace_name, root.short_path).rpartition("/")[0],
        tool = ctx.attr.tool,
        prelude = _HOME_PRELUDE,
    ), is_executable = True)
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = ctx.runfiles([launcher]).merge(erlang_runfiles))]

def _escript_binary_impl(ctx):
    archive = ctx.attr.upstream[DefaultInfo].files_to_run.executable
    (_, _, erlang_runfiles) = erlang_dirs(ctx)
    root = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo.version_file
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, otp_launcher_text(
        root = runfiles_path(ctx.workspace_name, root.short_path).rpartition("/")[0],
        tool = "escript",
        args = '"$runfiles/' + runfiles_path(ctx.workspace_name, archive.short_path) + '" ',
    ), is_executable = True)
    runfiles = ctx.runfiles([launcher, archive]).merge(erlang_runfiles).merge(ctx.attr.upstream[DefaultInfo].default_runfiles)
    for dep in ctx.attr.data:
        runfiles = runfiles.merge(dep[DefaultInfo].default_runfiles)
        runfiles = runfiles.merge(ctx.runfiles(transitive_files = dep[DefaultInfo].files))
    return [
        DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles),
        coverage_common.instrumented_files_info(ctx, source_attributes = ["srcs"], dependency_attributes = ["upstream"]),
        dx_quality_sources(ctx.files.srcs, _SPECS, str(ctx.label)),
    ]

_escript_binary = rule(
    implementation = _escript_binary_impl,
    doc = "Runs one escript archive on the resolved OTP runtime.",
    executable = True,
    attrs = dict({
        "srcs": attr.label_list(allow_files = _EXTS, doc = "Direct Erlang sources owned by this binary."),
        "upstream": attr.label(executable = True, cfg = "target", mandatory = True, doc = "The rules_erlang escript archive."),
    }, **dx_forward_data_attr()),
    provides = [DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    toolchains = ["@rules_erlang//tools:toolchain_type"],
)

def lcov_apps_dirs(short_paths):
    """Returns the COVERDATA_TO_LCOV_APPS_DIRS value for `<root>/<app>/src/<mod>.erl` paths."""
    roots = {}
    for path in short_paths:
        parts = path.split("/")
        if len(parts) < 4 or not path.endswith(".erl") or parts[-2] != "src":
            continue
        roots["/".join(parts[:-3])] = True
    return ":".join(sorted(roots.keys()))

def _erlang_test_coverage(ctx, upstream):
    """Returns coverage providers that put app sources where coverdata_to_lcov looks."""
    seen = {}
    for app in ctx.attr.apps + flat_deps(ctx.attr.apps):
        for f in app[ErlangAppInfo].srcs:
            if f.path.endswith(".erl") and f.owner != None and f.owner.workspace_name == "":
                seen[f] = True
    sources = sorted(seen.keys(), key = lambda f: f.short_path)
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["upstream", "apps"],
        metadata_files = sources,
    )
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    dirs = lcov_apps_dirs([f.short_path for f in sources])
    if dirs:
        env["COVERDATA_TO_LCOV_APPS_DIRS"] = dirs
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = None,
    )

_erlang_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _SPECS,
    what = "erlang test",
    allow_files = _EXTS,
    upstream_providers = None,
    extra_attrs = dx_lcov_merger_attr() | {"apps": attr.label_list(
        doc = "Erlang app targets whose sources feed test coverage.",
        providers = [_ErlangAppInfo],
    )},
    runtime = "besteffort",
    coverage_providers = _erlang_test_coverage,
)

def ct_suite_source(suite_name):
    """Returns the source file one Common Test suite compiles."""
    return "test/" + suite_name + ".erl"

def _is_label(entry):
    return entry.startswith(":") or entry.startswith("//") or entry.startswith("@")

def _listed(kwargs, attrs):
    entries = []
    for attr in attrs:
        if type(kwargs.get(attr)) == "list":
            entries += kwargs[attr]
    return entries

def app_sources(kwargs, glob = None):
    """Returns the source files one erlang_app owns for its attributes."""
    if kwargs.get("srcs") != None:
        return _listed(kwargs, ["srcs", "hdrs"])
    if kwargs.get("beam_files") != None:
        return []
    glob = glob if glob != None else native.glob
    excluded = [entry for entry in _listed(kwargs, ["extra_srcs", "extra_hdrs"]) if not _is_label(entry)]
    return glob(ERLANG_SOURCE_GLOBS, exclude = excluded, allow_empty = True)

def _app(upstream_macro, name, testonly, visibility, kwargs):
    tags = kwargs.pop("tags", None)
    owned = app_sources(kwargs)
    upstream_macro(name = name + "_upstream", testonly = testonly, tags = dx_upstream_tags(tags), **kwargs)
    forward_kwargs = {"testonly": True} if testonly else {}
    if tags != None:
        forward_kwargs["tags"] = tags
    _erlang_app_forward(
        name = name,
        upstream = ":" + name + "_upstream",
        srcs = owned,
        visibility = visibility,
        **forward_kwargs
    )

def erlang_app(name = "erlang_app", visibility = None, testonly = False, **kwargs):
    """Thin wrapper over the rules_erlang erlang_app macro."""
    _app(_erlang_app, name, testonly, visibility, kwargs)

def test_erlang_app(name = "test_erlang_app", visibility = None, **kwargs):
    """Thin wrapper over the rules_erlang test_erlang_app macro."""
    _app(_test_erlang_app, name, True, visibility, kwargs)

def erlang_bytecode(name, srcs, visibility = None, **kwargs):
    """Thin wrapper over rules_erlang erlang_bytecode."""
    dx_wrap(name, _erlang_bytecode, _erlang_bytecode_forward, srcs, visibility = visibility, **kwargs)

def escript_archive(name, visibility = None, **kwargs):
    """Thin wrapper over rules_erlang escript_archive that runs on the pinned runtime."""
    srcs = kwargs.get("srcs", [])
    _escript_archive(name = name + "_upstream", **dx_binary_upstream_kwargs(kwargs, srcs))
    _escript_binary(
        name = name,
        upstream = ":" + name + "_upstream",
        srcs = srcs,
        visibility = visibility,
        **dx_binary_forward_kwargs(kwargs)
    )

def eunit(name = "eunit", visibility = None, **kwargs):
    """Thin wrapper over the rules_erlang eunit test."""
    target = kwargs.get("target")
    apps = ([target] if target else []) + (kwargs.get("deps") or [])
    dx_wrap_test(name, _eunit, _erlang_forward_test, None, visibility = visibility, extra_forward_kwargs = {"apps": apps}, **kwargs)

LOCAL_NODE_ARGS = ["-sname", "undefined", "-start_epmd", "false"]

def ct_run_args(extra_args):
    """Returns ct_run arguments that keep a suite off the shared epmd unless it names its node."""
    if type(extra_args) != "list":
        return LOCAL_NODE_ARGS + extra_args
    words = " ".join(extra_args).split(" ")
    if "-sname" in words or "-name" in words:
        return extra_args
    return LOCAL_NODE_ARGS + extra_args

def ct_suite(name, suite_name = "", visibility = None, **kwargs):
    """Thin wrapper over the rules_erlang ct_suite macro."""
    suite = suite_name if suite_name != "" else name
    kwargs["ct_run_extra_args"] = ct_run_args(kwargs.get("ct_run_extra_args", []))

    # ct_run_extra_args names the node, so rules_erlang must not add -hidden -sname.
    kwargs = dx_setdefaults(kwargs, {"auto_sname": False})
    _ct_suite(name = name + "_upstream", suite_name = suite, **dx_test_upstream_kwargs(kwargs))
    _erlang_forward_test(
        name = name,
        testonly = True,
        upstream = ":" + name + "_upstream",
        srcs = [ct_suite_source(suite)],
        apps = [":test_erlang_app"] + (kwargs.get("deps") or []) + (kwargs.get("runtime_deps") or []),
        visibility = visibility,
        **dx_test_forward_kwargs(kwargs)
    )
