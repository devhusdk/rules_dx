"""Experimental minimal TypeScript wrappers."""

load("@aspect_rules_jest//jest:defs.bzl", _jest_test = "jest_test")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo")
load("@aspect_rules_ts//ts:defs.bzl", _TsConfigInfo = "TsConfigInfo", _ts_project = "ts_project")
load("@aspect_rules_ts//ts/private:ts_config.bzl", _write_tsconfig = "write_tsconfig")
load("//libs/starlark:wrapper.bzl", "dx_forward_attrs", "dx_forwarded_optional", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_quality_sources", "dx_symlink_default_info", "dx_test_forward_kwargs", "dx_wrap")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_TS_PROJECT_PROVIDES = [
    _JsInfo,
    _TsConfigInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_TS_TEST_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_DX_TS_SOURCE_SPECS = [
    ("typescript", ["ts", "mts", "cts"], [".d.ts", ".d.mts", ".d.cts"]),
    ("tsx", "tsx"),
]
_DX_TS_SOURCE_EXTS = [".ts", ".tsx", ".mts", ".cts"]

_typescript_project_forward = dx_library_forward_rule(
    provides = _DX_TS_PROJECT_PROVIDES,
    required_providers = [(_JsInfo, "JsInfo"), (_TsConfigInfo, "TsConfigInfo")],
    quality_specs = _DX_TS_SOURCE_SPECS,
    what = "typescript_*",
    allow_files = _DX_TS_SOURCE_EXTS,
    upstream_providers = [[_JsInfo]],
)

_DX_TS_DECLARATION_SUFFIXES = [".d.ts", ".d.mts", ".d.cts"]

def _is_declaration(src):
    """Returns whether a source path is an inert declaration file."""
    for suffix in _DX_TS_DECLARATION_SUFFIXES:
        if src.endswith(suffix):
            return True
    return False

def typescript_srcs_rejection(srcs):
    """Returns the rejection for forbidden typescript_project srcs, or None."""
    bad = [src for src in srcs or [] if _is_declaration(src)]
    if bad:
        return ("typescript_project takes real sources only; declaration " +
                "files are inert and must not be listed in srcs: " +
                ", ".join(sorted(bad)))
    return None

def _dx_scoped_tsconfig(name, srcs, kwargs):
    """Returns kwargs with a tsconfig that lists only this target's own sources."""
    tsconfig = kwargs.get("tsconfig", None)
    if tsconfig == None:
        return kwargs

    generated = "tsconfig_" + name
    _write_tsconfig(
        name = generated,
        config = {},
        files = srcs,
        extends = tsconfig,
        out = generated + ".json",
        visibility = ["//visibility:private"],
    )

    out = dict(kwargs)
    out["tsconfig"] = generated + ".json"
    out["extends"] = tsconfig
    out["isolated_typecheck"] = True
    return out

def _typescript_wrap_project(name, srcs, visibility = None, **kwargs):
    rejection = typescript_srcs_rejection(srcs)
    if rejection != None:
        fail(rejection)

    dx_wrap(name, _ts_project, _typescript_project_forward, srcs, visibility = visibility, **_dx_scoped_tsconfig(name, srcs, kwargs))

def typescript_project(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over ts_project."""
    _typescript_wrap_project(name, srcs, visibility = visibility, **kwargs)

def _typescript_test_forward_impl(ctx):
    upstream = ctx.attr.upstream

    env_inherit = list(ctx.attr.env_inherit) if ctx.attr.env_inherit else []
    if "TESTBRIDGE_TEST_ONLY" not in env_inherit:
        env_inherit.append("TESTBRIDGE_TEST_ONLY")
    out = [
        dx_symlink_default_info(ctx, "typescript_*"),
        testing.TestEnvironment({}, env_inherit),
        dx_quality_sources(ctx.files.srcs, _DX_TS_SOURCE_SPECS, str(ctx.label)),
    ]

    return out + dx_forwarded_optional(upstream, [InstrumentedFilesInfo, OutputGroupInfo], "typescript_*")

_typescript_test = rule(
    implementation = _typescript_test_forward_impl,
    test = True,
    provides = _DX_TS_TEST_PROVIDES,
    attrs = dx_forward_attrs(
        allow_files = _DX_TS_SOURCE_EXTS,
        upstream_providers = [[DefaultInfo]],
        extra_attrs = {
            "env_inherit": attr.string_list(),
        } | dx_lcov_merger_attr(),
    ),
)

def typescript_test_rejection(kwargs):
    """Returns the rejection for forbidden typescript_test kwargs, or None."""
    if kwargs.get("auto_configure_reporters", True) == False:
        return ("typescript_test always uses jest with the standard " +
                "auto-configured reporters (Bazel test logs); " +
                "`auto_configure_reporters = False` is not supported.")
    return None

def typescript_test_env(env_inherit):
    """Computes the effective test-runtime inherited environment."""
    env = list(env_inherit) if env_inherit != None else []
    if "TESTBRIDGE_TEST_ONLY" not in env:
        env.append("TESTBRIDGE_TEST_ONLY")
    return env

def typescript_test(name, srcs, node_modules, data = None, deps = None, tsconfig = None, transpiler = None, declaration = None, visibility = None, tags = None, env_inherit = None, **kwargs):
    """Experimental minimal wrapper over jest_test for TypeScript sources."""
    rejection = typescript_srcs_rejection(srcs)
    if rejection != None:
        fail(rejection)
    reporter_rejection = typescript_test_rejection(kwargs)
    if reporter_rejection != None:
        fail(reporter_rejection)
    effective_env = typescript_test_env(env_inherit)

    ts_kwargs = {}
    if deps != None:
        ts_kwargs["deps"] = list(deps)
    if tsconfig != None:
        ts_kwargs["tsconfig"] = tsconfig
    if transpiler != None:
        ts_kwargs["transpiler"] = transpiler
    if declaration != None:
        ts_kwargs["declaration"] = declaration
    _ts_project(
        name = name + "_ts",
        srcs = srcs,
        testonly = True,
        visibility = ["//visibility:private"],
        **_dx_scoped_tsconfig(name + "_ts", srcs, ts_kwargs)
    )

    upstream_data = [":" + name + "_ts"] + list(deps or []) + list(data or [])
    if "//:package_json" not in upstream_data:
        upstream_data.append("//:package_json")

    upstream_kwargs = dict(kwargs)
    upstream_kwargs.pop("aspect_hints", None)
    if tags != None:
        kept = [t for t in tags if t != "manual"]
        if len(kept) > 0:
            upstream_kwargs["tags"] = kept
        elif "tags" in upstream_kwargs:
            upstream_kwargs.pop("tags")
    elif "tags" in upstream_kwargs:
        upstream_kwargs.pop("tags")

    _jest_test(
        name = name + "_upstream",
        node_modules = node_modules,
        data = upstream_data,
        env_inherit = effective_env,
        visibility = ["//visibility:private"],
        **upstream_kwargs
    )
    forward_kwargs = dx_test_forward_kwargs(kwargs)
    forward_kwargs.pop("env", None)
    forward_kwargs.pop("flaky", None)
    _typescript_test(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        env_inherit = effective_env,
        visibility = visibility,
        tags = [t for t in tags if t != "manual"] if tags != None else forward_kwargs.pop("tags", None),
        **forward_kwargs
    )
