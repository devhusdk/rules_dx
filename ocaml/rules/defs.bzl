"""Thin OCaml wrappers over rules_ocaml."""

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
    _ocamlyacc = "ocamlyacc",
)
load("//libs/starlark:wrapper.bzl", "dx_append_flags", "dx_binary_forward_kwargs", "dx_binary_upstream_kwargs", "dx_forward_attrs", "dx_forward_data_attr", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_quality_sources", "dx_test_forward_kwargs", "dx_test_upstream_kwargs", "dx_upstream", "dx_upstream_tags", "dx_wrap_forward_kwargs")
load("//quality:sources.bzl", "QualitySourcesInfo")

ocamllex = _ocamllex

ocamlyacc = _ocamlyacc

OCAML_COMPATIBLE_WITH = [
    Label("@platforms//os:linux"),
    Label("@platforms//cpu:x86_64"),
]

OCAML_TOOLCHAIN_FLAG = "@rules_ocaml//toolchain"

UPSTREAM_DEFAULT_TOOLCHAIN = "ocamlopt.opt"

OCAML_DEFAULT_TOOLCHAIN = "ocamlc.opt"

OCAML_LINK_OPTS = ["-without-runtime"]

LOCAL_TAGS = ["no-remote-exec"]

SKIP_TAGS = ["no-format", "no-lint", "no-typecheck"]

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

def toolchain_choice(current):
    """Returns the rules_ocaml toolchain to build with; the bytecode compiler replaces the upstream default."""
    return OCAML_DEFAULT_TOOLCHAIN if current == UPSTREAM_DEFAULT_TOOLCHAIN else current

def _toolchain_impl(settings, _attr):
    return {OCAML_TOOLCHAIN_FLAG: toolchain_choice(settings[OCAML_TOOLCHAIN_FLAG])}

_toolchain = transition(
    implementation = _toolchain_impl,
    inputs = [OCAML_TOOLCHAIN_FLAG],
    outputs = [OCAML_TOOLCHAIN_FLAG],
)

def _library_forward_rule(provider, name):
    return dx_library_forward_rule(
        provides = [provider, DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
        required_providers = [(provider, name), (_OCamlDepsProvider, "OCamlDepsProvider")],
        quality_specs = _OCAML,
        what = "ocaml_*",
        allow_files = _SOURCE_EXTS,
        upstream_providers = [[provider]],
        runtime = "besteffort",
        quiet_providers = [p for p in _FORWARDED if p != provider],
        upstream_cfg = _toolchain,
    )

_module_forward = _library_forward_rule(_OCamlModuleProvider, "OCamlModuleProvider")

_signature_forward = _library_forward_rule(_OCamlSignatureProvider, "OCamlSignatureProvider")

_library_forward = _library_forward_rule(_OCamlLibraryProvider, "OCamlLibraryProvider")

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

def runfiles_path(workspace_name, short_path):
    """Returns one file's path below the runfiles root."""
    if short_path.startswith("../"):
        return short_path[3:]
    return workspace_name + "/" + short_path

def run_environment(upstream_env, upstream_inherited, env, inherited):
    """Returns the forwarder's run environment: upstream values, then the wrapper's own env and env_inherit."""
    merged = dict(upstream_env)
    merged.update(env)
    names = list(upstream_inherited)
    names += [key for key in inherited if key not in names]
    return struct(environment = merged, inherited_environment = names)

def _run_environment_info(ctx, upstream):
    upstream_env, upstream_inherited = {}, []
    if RunEnvironmentInfo in upstream:
        upstream_env = upstream[RunEnvironmentInfo].environment
        upstream_inherited = upstream[RunEnvironmentInfo].inherited_environment
    out = run_environment(upstream_env, upstream_inherited, ctx.attr.env, ctx.attr.env_inherit)
    return RunEnvironmentInfo(environment = out.environment, inherited_environment = out.inherited_environment)

def _launcher_impl(ctx):
    upstream = dx_upstream(ctx)
    info = upstream[DefaultInfo]
    bytecode = info.files_to_run.executable
    if bytecode == None:
        fail("ocaml_*: upstream target has no executable: " + str(upstream.label))
    runtime = ctx.attr._ocamlrun[DefaultInfo]
    ocamlrun = runtime.files.to_list()[0]
    launcher = ctx.actions.declare_file(ctx.label.name)
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

def _launcher_attrs(extra = None):
    attrs = dx_forward_attrs(allow_files = _SOURCE_EXTS, upstream_providers = None, extra_attrs = extra, upstream_cfg = _toolchain)
    attrs.update(dx_forward_data_attr())
    attrs["env"] = attr.string_dict(doc = "Environment variables set when the executable runs.")
    attrs["env_inherit"] = attr.string_list(doc = "Environment variable names passed through from the caller.")
    attrs["_ocamlrun"] = attr.label(default = "@dx_ocaml_toolchains//:ocamlrun", allow_files = True)
    return attrs

_ocaml_forward_binary = rule(
    implementation = _launcher_impl,
    doc = "Runs one rules_ocaml bytecode executable on the pinned ocamlrun.",
    executable = True,
    provides = [DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    attrs = _launcher_attrs(),
)

_ocaml_forward_test = rule(
    implementation = _launcher_impl,
    doc = "Runs one rules_ocaml bytecode test on the pinned ocamlrun.",
    test = True,
    provides = [DefaultInfo, InstrumentedFilesInfo, QualitySourcesInfo],
    attrs = _launcher_attrs(dx_lcov_merger_attr()),
)

def owned_sources(*files):
    """Returns the hand-written .ml and .mli files among the given labels."""
    return [f for f in files if type(f) == "string" and (f.endswith(".ml") or f.endswith(".mli"))]

def upstream_tags(tags):
    """Returns private upstream tags that keep OCaml actions on this host."""
    out = dx_upstream_tags(tags)
    return out + [tag for tag in LOCAL_TAGS if tag not in out]

def with_defaults(kwargs):
    """Returns kwargs with the qualified OCaml cell and the quality skip tags as defaults."""
    out = dict(kwargs)
    if out.get("target_compatible_with") == None:
        out["target_compatible_with"] = OCAML_COMPATIBLE_WITH
    tags = list(out.get("tags") or [])
    out["tags"] = tags + [tag for tag in SKIP_TAGS if tag not in tags]
    return out

def link_kwargs(kwargs):
    """Returns binary kwargs that link plain bytecode for the runfiles ocamlrun."""
    out = dx_append_flags(kwargs, "opts", OCAML_LINK_OPTS)
    if out.get("vm_linkage") == None:
        out["vm_linkage"] = "dynamic"
    return out

TEST_ONLY_ATTRS = ["env", "env_inherit", "flaky", "local", "shard_count", "size", "testonly", "timeout"]

RUN_ENV_ATTRS = ["env", "env_inherit"]

def test_upstream_kwargs(kwargs):
    """Returns the test kwargs the upstream executable accepts."""
    return {key: value for key, value in kwargs.items() if key not in TEST_ONLY_ATTRS}

def test_forward_kwargs(kwargs):
    """Returns the test forwarder kwargs, including env and env_inherit, which the forwarder applies itself."""
    out = dx_test_forward_kwargs(kwargs)
    for key in RUN_ENV_ATTRS:
        if kwargs.get(key) != None:
            out[key] = kwargs[key]
    return out

def default_module_name(name, sig):
    """Returns the module_name default: the target name unless sig names a signature target."""
    if sig == None or (type(sig) == "string" and sig.endswith(".mli")):
        return name
    return None

def _library_shape(name, upstream_rule, forward_rule, srcs, visibility, kwargs):
    kwargs = with_defaults(kwargs)
    upstream = dict(kwargs)
    upstream["tags"] = upstream_tags(kwargs.get("tags"))
    upstream_rule(name = name + "_upstream", visibility = ["//visibility:private"], **upstream)
    forward_rule(name = name, upstream = ":" + name + "_upstream", srcs = srcs, visibility = visibility, **dx_wrap_forward_kwargs(kwargs))

def ocaml_module(name, struct, sig = None, visibility = None, **kwargs):
    """Thin wrapper over the rules_ocaml ocaml_module rule."""
    if sig != None:
        kwargs["sig"] = sig
    if kwargs.get("module_name") == None:
        module_name = default_module_name(name, sig)
        if module_name != None:
            kwargs["module_name"] = module_name
    _library_shape(name, _ocaml_module, _module_forward, owned_sources(struct, sig), visibility, dict(kwargs, struct = struct))

def ocaml_signature(name, src, visibility = None, **kwargs):
    """Thin wrapper over the rules_ocaml ocaml_signature rule."""
    _library_shape(name, _ocaml_signature, _signature_forward, owned_sources(src), visibility, dict(kwargs, src = src))

def ocaml_library(name, visibility = None, **kwargs):
    """Thin wrapper over the rules_ocaml ocaml_library rule."""
    _library_shape(name, _ocaml_library, _library_forward, [], visibility, kwargs)

def ocaml_binary(name, visibility = None, **kwargs):
    """Thin wrapper over the rules_ocaml ocaml_binary rule run on the pinned ocamlrun."""
    kwargs = with_defaults(kwargs)
    upstream = dx_binary_upstream_kwargs(link_kwargs(kwargs), None)
    upstream["tags"] = upstream_tags(kwargs.get("tags"))
    _ocaml_binary(name = name + "_upstream", **upstream)
    _ocaml_forward_binary(name = name, upstream = ":" + name + "_upstream", srcs = [], visibility = visibility, **dx_binary_forward_kwargs(kwargs))

def ocaml_test(name, visibility = None, **kwargs):
    """Builds one rules_ocaml bytecode executable and runs it as a test on the pinned ocamlrun."""
    kwargs = with_defaults(kwargs)
    upstream = test_upstream_kwargs(dx_test_upstream_kwargs(link_kwargs(kwargs)))
    upstream["tags"] = upstream_tags(kwargs.get("tags"))
    _ocaml_binary(name = name + "_upstream", testonly = True, **upstream)
    _ocaml_forward_test(name = name, testonly = True, upstream = ":" + name + "_upstream", srcs = [], visibility = visibility, **test_forward_kwargs(kwargs))
