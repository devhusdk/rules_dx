"""Thin OCaml wrappers over rules_ocaml."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load(
    "@rules_ocaml//build:providers.bzl",
    _OCamlArchiveProvider = "OCamlArchiveProvider",
    _OCamlCcInfo = "OCamlCcInfo",
    _OCamlCodepsProvider = "OCamlCodepsProvider",
    _OCamlDepsProvider = "OCamlDepsProvider",
    _OCamlLibraryProvider = "OCamlLibraryProvider",
    _OCamlModuleProvider = "OCamlModuleProvider",
    _OCamlNsResolverProvider = "OCamlNsResolverProvider",
    _OCamlSignatureProvider = "OCamlSignatureProvider",
    _OcamlNsSubmoduleMarker = "OcamlNsSubmoduleMarker",
)
load(
    "@rules_ocaml//build:rules.bzl",
    _ocaml_binary = "ocaml_binary",
    _ocaml_library = "ocaml_library",
    _ocaml_module = "ocaml_module",
    _ocaml_signature = "ocaml_signature",
    _ocamllex = "ocamllex",
)
load("//core:rlocation.bzl", "runfiles_path")
load("//cc:defs.bzl", "CC_COVERAGE_INPUTS", "cc_coverage_settings")
load("//core:fuzz.bzl", "dx_fuzz_runner")
load("//core:fuzz_bench.bzl", "merge_tags", "split_test_kwargs", "with_default_size")
load("//core:wrapper.bzl", "dx_binary_forward_kwargs", "dx_binary_upstream_kwargs", "dx_forward_attrs", "dx_forward_data_attr", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_quality_sources", "dx_run_env_attrs", "dx_run_environment", "dx_test_forward_kwargs", "dx_test_upstream_kwargs", "dx_upstream", "dx_warnings_as_errors", "dx_wrap_forward_kwargs")
load("//core:native_coverage.bzl", "dx_native_coverage_attrs", "dx_native_coverage_toolchains", "dx_native_test_coverage")
load("//core/quality:sources.bzl", "QualitySourcesInfo")
load(":kwargs.bzl", "ocaml_coverage_kwargs", "ocaml_default_module_name", "ocaml_link_kwargs", "ocaml_test_upstream_kwargs", "ocaml_upstream_tags", "ocaml_with_defaults", _OCAML_COMPATIBLE_WITH = "OCAML_COMPATIBLE_WITH")
load(":providers.bzl", "OCamlCmtInfo", "WARNINGS_AS_ERRORS_OPTS", "bin_annot_opts", "cmt_entries", "profile_warning_opts")

OCAML_COMPATIBLE_WITH = _OCAML_COMPATIBLE_WITH

# Re-exported unchanged: BUILD files need it to generate lexers.
ocamllex = _ocamllex

OCAML_TOOLCHAIN_FLAG = "@rules_ocaml//toolchain"

OCAML_EMITTER_FLAG = str(Label("//ocaml/private/toolchain:emitter"))

_FEATURES = "//command_line_option:features"

NATIVE_TOOLCHAIN = "ocamlopt.opt"

UPSTREAM_DEFAULT_TOOLCHAIN = "ocamlopt.opt"

OCAML_DEFAULT_TOOLCHAIN = "ocamlc.opt"

_OCAML = [("ocaml", ["ml", "mli"])]

_SOURCE_EXTS = [".ml", ".mli"]

_FORWARDED = [
    CcInfo,
    _OCamlArchiveProvider,
    _OCamlCcInfo,
    _OCamlCodepsProvider,
    _OCamlLibraryProvider,
    _OCamlModuleProvider,
    _OCamlNsResolverProvider,
    _OCamlSignatureProvider,
    _OcamlNsSubmoduleMarker,
]

def toolchain_choice(current, emitter):
    """Returns the rules_ocaml toolchain to build with: the emitter flag picks the native or the bytecode cell, and an explicit toolchain is kept."""
    if emitter == "sys":
        return NATIVE_TOOLCHAIN
    return OCAML_DEFAULT_TOOLCHAIN if current == UPSTREAM_DEFAULT_TOOLCHAIN else current

def toolchain_settings(settings):
    """Returns the toolchain choice, plus LLVM coverage features for the C stubs a native link takes in."""
    emitter = settings[OCAML_EMITTER_FLAG]
    out = {OCAML_TOOLCHAIN_FLAG: toolchain_choice(settings[OCAML_TOOLCHAIN_FLAG], emitter)}
    if emitter == "sys":
        out.update(cc_coverage_settings(settings))
    else:
        out[_FEATURES] = settings[_FEATURES]
    return out

def _toolchain_impl(settings, _attr):
    return toolchain_settings(settings)

_toolchain = transition(
    implementation = _toolchain_impl,
    inputs = [OCAML_TOOLCHAIN_FLAG, OCAML_EMITTER_FLAG] + CC_COVERAGE_INPUTS,
    outputs = [OCAML_TOOLCHAIN_FLAG, _FEATURES],
)

def _native_impl(settings, _attr):
    return dict(toolchain_settings(dict(settings, **{OCAML_EMITTER_FLAG: "sys"})), **{OCAML_EMITTER_FLAG: "sys"})

_native = transition(
    implementation = _native_impl,
    inputs = [OCAML_TOOLCHAIN_FLAG, OCAML_EMITTER_FLAG] + CC_COVERAGE_INPUTS,
    outputs = [OCAML_TOOLCHAIN_FLAG, OCAML_EMITTER_FLAG, _FEATURES],
)

def _cmt_providers(ctx, upstream):
    groups = upstream[OutputGroupInfo]
    cmts = groups.cmt.to_list() if hasattr(groups, "cmt") else []
    cmtis = groups.cmti.to_list() if hasattr(groups, "cmti") else []
    return [OCamlCmtInfo(owned = cmt_entries(ctx.files.srcs, cmts, cmtis))]

def _library_cmt_providers(ctx, _upstream):
    owned = []
    for member in ctx.attr.manifest:
        if OCamlCmtInfo in member:
            owned += member[OCamlCmtInfo].owned
    return [OCamlCmtInfo(owned = owned)]

def _library_forward_rule(provider, name, extra_providers = None, extra_attrs = None):
    return dx_library_forward_rule(
        extra_attrs = extra_attrs,
        extra_providers = extra_providers,
        provides = [provider, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo] + ([OCamlCmtInfo] if extra_providers else []),
        required_providers = [(provider, name), (_OCamlDepsProvider, "OCamlDepsProvider")],
        quality_specs = _OCAML,
        what = "ocaml_*",
        allow_files = _SOURCE_EXTS,
        upstream_providers = [[provider]],
        runtime = "besteffort",
        instrument_srcs = True,
        quiet_providers = [p for p in _FORWARDED if p != provider],
        upstream_cfg = _toolchain,
    )

_module_forward = _library_forward_rule(_OCamlModuleProvider, "OCamlModuleProvider", _cmt_providers, {"instrument_coverage": attr.bool(doc = "Whether to instrument the module for coverage.", default = True)})

_signature_forward = _library_forward_rule(_OCamlSignatureProvider, "OCamlSignatureProvider", _cmt_providers)

_library_forward = _library_forward_rule(_OCamlLibraryProvider, "OCamlLibraryProvider", _library_cmt_providers, {"manifest": attr.label_list(cfg = _toolchain, doc = "The modules and libraries the library bundles; their typed trees are forwarded in OCamlCmtInfo.")})

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
exec "$runfiles/{ocamlrun}" "$runfiles/{bytecode}" "$@"
"""

def launcher_script(ocamlrun, bytecode):
    """Returns the launcher that runs one bytecode file on the runfiles ocamlrun."""
    return _LAUNCHER.format(ocamlrun = ocamlrun, bytecode = bytecode)

def _run_environment_info(ctx, upstream):
    return dx_run_environment(ctx, upstream[RunEnvironmentInfo] if RunEnvironmentInfo in upstream else None)

def _native_coverage(ctx, upstream, exe):
    native = dx_native_test_coverage(ctx, upstream)
    metadata = native.instrumented.metadata_files.to_list()
    if exe != None and not ctx.target_platform_has_constraint(ctx.attr._macos[platform_common.ConstraintValueInfo]):
        objects = ctx.actions.declare_file(ctx.label.name + "_runtime_objects_list.txt")
        ctx.actions.write(objects, exe.short_path + "\n")
        metadata.append(objects)
    return struct(
        env = {key: native.env.environment[key] for key in ["CC_CODE_COVERAGE_SCRIPT", "LLVM_COV", "LLVM_PROFDATA"]},
        metadata = metadata,
    )

def _coverage_launcher(ctx, upstream, inner, files, native, runtime_runfiles = []):
    info = upstream[DefaultInfo]
    runfiles = ctx.runfiles(files).merge_all(
        [info.default_runfiles] + runtime_runfiles + [dep[DefaultInfo].default_runfiles for dep in ctx.attr.data],
    )
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = launcher, target_file = ctx.executable._bisect_collect, is_executable = True)
    sources = [f for f in upstream[InstrumentedFilesInfo].instrumented_files.to_list() if f.extension == "ml" and not f.short_path.startswith("../")]
    manifest = ctx.actions.declare_file(ctx.label.name + ".bisect_sources")
    ctx.actions.write(manifest, "".join([f.short_path + "\n" for f in sources]))
    report = ctx.attr._bisect_report[DefaultInfo]
    to_lcov = ctx.attr._bisect_to_lcov[DefaultInfo]
    env = dict(_run_environment_info(ctx, upstream).environment)
    env.update(native.env)
    env.update({
        "DX_BISECT_REPORT": runfiles_path(ctx.workspace_name, report.files_to_run.executable.short_path),
        "DX_BISECT_SOURCES": runfiles_path(ctx.workspace_name, manifest.short_path),
        "DX_BISECT_UPSTREAM": runfiles_path(ctx.workspace_name, inner.short_path),
        "DX_BISECT_WORKSPACE": ctx.workspace_name,
        "DX_TO_LCOV": runfiles_path(ctx.workspace_name, to_lcov.files_to_run.executable.short_path),
    })
    runfiles = runfiles.merge(ctx.runfiles([launcher, manifest, report.files_to_run.executable] + sources)).merge_all([
        report.default_runfiles,
        to_lcov.default_runfiles,
        ctx.attr._bisect_collect[DefaultInfo].default_runfiles,
    ])
    return [
        DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles),
        coverage_common.instrumented_files_info(ctx, dependency_attributes = ["upstream"], metadata_files = native.metadata),
        dx_quality_sources(ctx.files.srcs, _OCAML, str(ctx.label)),
        RunEnvironmentInfo(environment = env, inherited_environment = _run_environment_info(ctx, upstream).inherited_environment),
    ]

def _bytecode_coverage_launcher(ctx, upstream, bytecode):
    runtime = ctx.attr._ocamlrun[DefaultInfo]
    ocamlrun = runtime.files.to_list()[0]
    inner = ctx.actions.declare_file(ctx.label.name + ".run")
    ctx.actions.write(
        inner,
        launcher_script(runfiles_path(ctx.workspace_name, ocamlrun.short_path), runfiles_path(ctx.workspace_name, bytecode.short_path)),
        is_executable = True,
    )
    return _coverage_launcher(ctx, upstream, inner, [inner, bytecode, ocamlrun], _native_coverage(ctx, upstream, None), [runtime.default_runfiles])

def _launcher_impl(ctx):
    upstream = dx_upstream(ctx)
    info = upstream[DefaultInfo]
    bytecode = info.files_to_run.executable
    if bytecode == None:
        fail("ocaml_*: upstream target has no executable: " + str(upstream.label))
    sys = ctx.attr._emitter[BuildSettingInfo].value == "sys"
    if ctx.configuration.coverage_enabled and hasattr(ctx.attr, "_bisect_collect") and ctx.attr._bisect_report[DefaultInfo].files_to_run.executable != None:
        if sys:
            return _coverage_launcher(ctx, upstream, bytecode, [bytecode], _native_coverage(ctx, upstream, bytecode))
        return _bytecode_coverage_launcher(ctx, upstream, bytecode)
    launcher = ctx.actions.declare_file(ctx.label.name)
    if sys:
        ctx.actions.symlink(output = launcher, target_file = bytecode, is_executable = True)
        runfiles = ctx.runfiles([launcher, bytecode]).merge_all(
            [info.default_runfiles] + [dep[DefaultInfo].default_runfiles for dep in ctx.attr.data],
        )
        return [
            DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles),
            coverage_common.instrumented_files_info(ctx, dependency_attributes = ["upstream"]),
            dx_quality_sources(ctx.files.srcs, _OCAML, str(ctx.label)),
            _run_environment_info(ctx, upstream),
        ]
    runtime = ctx.attr._ocamlrun[DefaultInfo]
    ocamlrun = runtime.files.to_list()[0]
    ctx.actions.write(
        launcher,
        launcher_script(runfiles_path(ctx.workspace_name, ocamlrun.short_path), runfiles_path(ctx.workspace_name, bytecode.short_path)),
        is_executable = True,
    )
    runfiles = ctx.runfiles([launcher, bytecode, ocamlrun]).merge_all(
        [info.default_runfiles, runtime.default_runfiles] + [dep[DefaultInfo].default_runfiles for dep in ctx.attr.data],
    )
    return [
        DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles),
        coverage_common.instrumented_files_info(ctx, dependency_attributes = ["upstream"]),
        dx_quality_sources(ctx.files.srcs, _OCAML, str(ctx.label)),
        _run_environment_info(ctx, upstream),
    ]

def _launcher_attrs(extra = None, upstream_aspects = None):
    attrs = dx_forward_attrs(allow_files = _SOURCE_EXTS, upstream_providers = None, extra_attrs = extra, upstream_cfg = _toolchain, upstream_aspects = upstream_aspects)
    attrs.update(dx_forward_data_attr())
    attrs.update(dx_run_env_attrs())
    attrs["_emitter"] = attr.label(default = OCAML_EMITTER_FLAG, providers = [BuildSettingInfo], doc = "The emitter flag: sys links native executables, vm bytecode.")
    attrs["_ocamlrun"] = attr.label(default = "@dx_ocaml_toolchains//:ocamlrun", allow_files = True)
    return attrs

_ocaml_forward_binary = rule(
    implementation = _launcher_impl,
    doc = "Runs one rules_ocaml bytecode executable on the pinned ocamlrun.",
    executable = True,
    provides = [DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    attrs = _launcher_attrs(),
)

def _test_attrs():
    return _launcher_attrs(dx_lcov_merger_attr() | dx_native_coverage_attrs() | {
        "_bisect_collect": attr.label(default = "//ocaml/private/coverage:collect", executable = True, cfg = "target"),
        "_bisect_report": attr.label(default = "//ocaml/private/coverage:report_or_none", cfg = "target"),
        "_bisect_to_lcov": attr.label(default = "//ocaml/private/coverage:to_lcov_or_none", cfg = "target"),
        "_macos": attr.label(default = "@platforms//os:macos"),
    })

_ocaml_forward_test = rule(
    implementation = _launcher_impl,
    doc = "Runs one rules_ocaml bytecode test on the pinned ocamlrun.",
    test = True,
    provides = [DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    toolchains = dx_native_coverage_toolchains(),
    attrs = _test_attrs(),
)

_ocaml_forward_bench_test = rule(
    implementation = _launcher_impl,
    doc = "Runs one rules_ocaml native benchmark: the target and everything below it build with the sys emitter.",
    test = True,
    cfg = _native,
    provides = [DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    toolchains = dx_native_coverage_toolchains(),
    attrs = _test_attrs(),
)

def owned_sources(*files):
    """Returns the hand-written .ml and .mli files among the given labels."""
    return [f for f in files if type(f) == "string" and (f.endswith(".ml") or f.endswith(".mli"))]

def _library_shape(name, upstream_rule, forward_rule, srcs, visibility, kwargs, forward_extra = {}):
    kwargs = ocaml_with_defaults(kwargs)
    upstream = dict(kwargs)
    upstream["tags"] = ocaml_upstream_tags(kwargs.get("tags"))
    upstream_rule(name = name + "_upstream", visibility = ["//visibility:private"], **upstream)
    forward_rule(name = name, upstream = ":" + name + "_upstream", srcs = srcs, visibility = visibility, **dict(dx_wrap_forward_kwargs(kwargs), **forward_extra))

def ocaml_module(name, struct, sig = None, visibility = None, instrument_coverage = True, coverage_ppx = False, **kwargs):
    """Thin wrapper over the rules_ocaml ocaml_module rule; modules are instrumented for bisect_ppx coverage unless instrument_coverage is False, and coverage_ppx marks a module whose own ppx already links bisect_ppx."""
    if sig != None:
        kwargs["sig"] = sig
    if kwargs.get("module_name") == None:
        module_name = ocaml_default_module_name(name, sig)
        if module_name != None:
            kwargs["module_name"] = module_name
    kwargs, warning_flags = dx_warnings_as_errors(kwargs, WARNINGS_AS_ERRORS_OPTS)
    kwargs["opts"] = profile_warning_opts(bin_annot_opts(kwargs.get("opts"))) + warning_flags
    if instrument_coverage:
        kwargs = ocaml_coverage_kwargs(kwargs, struct, native.package_name(), coverage_ppx)
    _library_shape(name, _ocaml_module, _module_forward, owned_sources(struct, sig), visibility, dict(kwargs, struct = struct), {"instrument_coverage": instrument_coverage})

def ocaml_signature(name, src, visibility = None, **kwargs):
    """Thin wrapper over the rules_ocaml ocaml_signature rule."""
    kwargs, warning_flags = dx_warnings_as_errors(kwargs, WARNINGS_AS_ERRORS_OPTS)
    kwargs["opts"] = profile_warning_opts(bin_annot_opts(kwargs.get("opts"))) + warning_flags
    _library_shape(name, _ocaml_signature, _signature_forward, owned_sources(src), visibility, dict(kwargs, src = src))

def ocaml_library(name, visibility = None, **kwargs):
    """Thin wrapper over the rules_ocaml ocaml_library rule."""
    _library_shape(name, _ocaml_library, _library_forward, [], visibility, kwargs, {"manifest": kwargs.get("manifest", [])})

def ocaml_binary(name, *, visibility = None, **kwargs):
    """Binary on the pinned ocamlrun, compatible only with the qualified OCaml cells."""
    kwargs = ocaml_with_defaults(kwargs)
    upstream = dx_binary_upstream_kwargs(ocaml_link_kwargs(kwargs), None)
    upstream["tags"] = ocaml_upstream_tags(kwargs.get("tags"))
    _ocaml_binary(name = name + "_upstream", **upstream)
    _ocaml_forward_binary(name = name, upstream = ":" + name + "_upstream", srcs = [], visibility = visibility, **dx_binary_forward_kwargs(kwargs))

def ocaml_test(name, *, visibility = None, **kwargs):
    """Builds one rules_ocaml bytecode executable and runs it as a test on the pinned ocamlrun."""
    _test_shape(name, _ocaml_forward_test, visibility, kwargs)

def _test_shape(name, forward_rule, visibility, kwargs):
    kwargs = ocaml_with_defaults(kwargs)
    upstream = ocaml_test_upstream_kwargs(dx_test_upstream_kwargs(ocaml_link_kwargs(kwargs)))
    upstream["tags"] = ocaml_upstream_tags(kwargs.get("tags"))
    _ocaml_binary(name = name + "_upstream", testonly = True, **upstream)
    forward_rule(name = name, testonly = True, upstream = ":" + name + "_upstream", srcs = [], visibility = visibility, **dx_test_forward_kwargs(kwargs))

def ocaml_fuzz_test(name, *, main, deps = None, visibility = None, tags = None, **kwargs):
    """A Crowbar test in quickcheck mode: `deps` must hold the crowbar package, and the test takes Crowbar's flags such as `-r` in `args`. It is manual, tagged `dx-fuzz`, and `<name>_run` drives it for `dx fuzz`."""
    build, run = split_test_kwargs(kwargs)
    ocaml_module(
        name = name + "_bin",
        testonly = True,
        struct = main,
        deps = deps,
        tags = merge_tags(tags, "manual"),
        visibility = ["//visibility:private"],
        **build
    )
    ocaml_test(
        name = name,
        main = ":" + name + "_bin",
        tags = merge_tags(tags, "dx-fuzz", "manual"),
        visibility = visibility,
        **with_default_size(run)
    )
    dx_fuzz_runner(name, ":" + name, engine = "crowbar", visibility = visibility)

def ocaml_bench(name, *, main, deps = None, visibility = None, tags = None, **kwargs):
    """A benchmark `dx bench` runs: `main` calls `Dx_bench.run`, which writes `dx-bench.json`. It is manual and tagged `dx-bench`."""
    build, run = split_test_kwargs(kwargs)
    ocaml_module(
        name = name + "_bin",
        testonly = True,
        struct = main,
        deps = (deps or []) + [Label("//ocaml/bench:dx_bench")],
        tags = merge_tags(tags, "manual"),
        visibility = ["//visibility:private"],
        **build
    )
    _test_shape(name, _ocaml_forward_bench_test, visibility, dict(
        main = ":" + name + "_bin",
        tags = merge_tags(tags, "dx-bench", "manual"),
        **with_default_size(run)
    ))
