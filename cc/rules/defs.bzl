"""Minimal C, C++ and Objective-C wrappers."""

load("@rules_cc//cc:defs.bzl", _cc_binary = "cc_binary", _cc_library = "cc_library", _cc_test = "cc_test")
load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain")
load("@rules_cc//cc:objc_library.bzl", _objc_library = "objc_library")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("//cc/toolchains:bindings.bzl", "cc_compiler_identity")
load("//libs/starlark:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_executable_forward_rule", "dx_append_flags", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_native_coverage_attrs", "dx_native_coverage_toolchains", "dx_native_test_coverage", "dx_warnings_as_errors", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//rust/rules:msvc_crt.bzl", "dx_msvc_crt_aspect", "dx_msvc_crt_reset_transition", "dx_msvc_crt_validate")
load("//quality:sources.bzl", "QualitySourcesInfo", "UPSTREAM_CC_CLASS_EXTENSIONS")

_DX_CC_LIBRARY_PROVIDES = [
    CcInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_CC_SOURCE_SPECS = [(k, UPSTREAM_CC_CLASS_EXTENSIONS[k]) for k in ("c", "cpp")]

_cc_library_forward = dx_library_forward_rule(
    provides = _DX_CC_LIBRARY_PROVIDES,
    required_providers = [(CcInfo, "CcInfo")],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = True,
    upstream_providers = [[CcInfo]],
    extra_attrs = {
        "hdrs": attr.label_list(
            doc = "Header files exported to dependent targets.",
            allow_files = True,
        ),
    },
    extra_quality_attrs = ["hdrs"],
)

_cc_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = True,
    upstream_providers = [[CcInfo]],
    optional_providers = [CcInfo],
    runtime = "besteffort",
    upstream_aspects = [dx_msvc_crt_aspect],
    validate = dx_msvc_crt_validate,
    cfg = dx_msvc_crt_reset_transition,
)

_LLVM_COVERAGE_FEATURES = ["llvm_coverage_map_format", "-gcc_coverage_map_format"]

_COMPILER_FLAG = str(Label("//cc/toolchains:compiler"))

def cc_coverage_features(features, collect_code_coverage, compiler = ""):
    """Returns the features a cc_test builds with: LLVM coverage maps only under coverage and an LLVM or unset compiler."""
    if not collect_code_coverage:
        return features
    if compiler and cc_compiler_identity(compiler, "").coverage != "llvm":
        return features
    return features + [f for f in _LLVM_COVERAGE_FEATURES if f not in features]

CC_COVERAGE_INPUTS = ["//command_line_option:features", "//command_line_option:collect_code_coverage", "//command_line_option:compiler", _COMPILER_FLAG]

def cc_coverage_settings(settings):
    """Returns the features setting a transition over CC_COVERAGE_INPUTS gives C and C++ code built for coverage."""
    features = cc_coverage_features(
        settings["//command_line_option:features"],
        settings["//command_line_option:collect_code_coverage"],
        "gcc" if settings[_COMPILER_FLAG] == "gcc" else settings["//command_line_option:compiler"] or "",
    )
    return {"//command_line_option:features": features}

def _cc_coverage_transition_impl(settings, _attr):
    return cc_coverage_settings(settings)

_cc_coverage_transition = transition(
    implementation = _cc_coverage_transition_impl,
    inputs = CC_COVERAGE_INPUTS,
    outputs = ["//command_line_option:features"],
)

def cc_coverage_unsupported(compiler):
    """Returns why native coverage cannot run on a known non-LLVM compiler, or an empty string."""
    if not compiler:
        return ""
    identity = cc_compiler_identity(compiler, "")
    if identity.family == "unknown" or identity.coverage in ("llvm", "gcov"):
        return ""
    return "cc_test coverage needs an LLVM compiler (clang or clang-cl); the toolchain compiler is '%s'. Run without --collect_code_coverage or select an LLVM toolchain." % compiler

def cc_coverage_mismatch(compiler, llvm_maps):
    """Returns why a known non-LLVM compiler cannot use the LLVM coverage features the transition added, or an empty string."""
    if not llvm_maps or not compiler:
        return ""
    identity = cc_compiler_identity(compiler, "")
    if identity.family == "unknown" or identity.coverage == "llvm":
        return ""
    hint = "Pass --compiler=%s" % compiler
    if compiler == "gcc":
        hint += " or --@rules_dx//cc/toolchains:compiler=gcc"
    return "cc_test coverage under the '%s' toolchain needs %s coverage maps, but LLVM coverage features were added. %s." % (compiler, identity.coverage, hint)

CcCoverageProbeInfo = provider(doc = "Whether LLVM coverage maps are enabled in the cc_test configuration.", fields = {"llvm": "True when llvm_coverage_map_format is a feature."})

def _cc_coverage_probe_impl(ctx):
    return [CcCoverageProbeInfo(llvm = "llvm_coverage_map_format" in ctx.features)]

cc_coverage_probe = rule(implementation = _cc_coverage_probe_impl, doc = "Reports whether LLVM coverage maps are enabled in the test configuration.")

def _cc_test_coverage(ctx, upstream):
    toolchain = find_cc_toolchain(ctx, mandatory = False)
    reason = cc_coverage_unsupported(toolchain.compiler if toolchain else "")
    if reason:
        fail(reason)
    mismatch = cc_coverage_mismatch(toolchain.compiler if toolchain else "", ctx.attr._coverage_probe[0][CcCoverageProbeInfo].llvm)
    if mismatch:
        fail(mismatch)
    gcov = bool(toolchain) and cc_compiler_identity(toolchain.compiler, "").coverage == "gcov"
    return dx_native_test_coverage(ctx, upstream, gcov = gcov)

CC_COVERAGE_TRANSITION = _cc_coverage_transition

_cc_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_CC_SOURCE_SPECS,
    what = "cc_*",
    allow_files = True,
    upstream_providers = [[CcInfo]],
    extra_attrs = dx_lcov_merger_attr() | dx_native_coverage_attrs() | {
        "_coverage_probe": attr.label(default = "//cc/rules:coverage_probe", cfg = _cc_coverage_transition),
    },
    optional_providers = [CcInfo],
    upstream_cfg = _cc_coverage_transition,
    coverage_providers = _cc_test_coverage,
    toolchains = dx_native_coverage_toolchains(),
)

def _cc_warnings_as_errors(kwargs):
    rest, flag = dx_warnings_as_errors(kwargs, ["treat_warnings_as_errors"])
    return dx_append_flags(rest, "features", flag)

_PARAM_FILE_ATTRS = ["textual_hdrs", "includes", "copts", "conlyopts", "cxxopts", "defines", "local_defines"]

def _needs_quoting(value):
    return type(value) == "string" and any([c in value for c in [" ", "\t", "\"", "'"]])

def cc_param_file_features(kwargs, srcs, hdrs):
    """Returns ["-compiler_param_file"] when the target's own sources or flags hold a space, quote or tab, else []."""
    values = [srcs, hdrs] + [kwargs.get(attr) for attr in _PARAM_FILE_ATTRS]
    for value in values:
        if type(value) in ("list", "tuple") and any([_needs_quoting(v) for v in value]):
            return ["-compiler_param_file"]
    return []

def _cc_kwargs(kwargs, srcs, hdrs = None):
    kwargs = _cc_warnings_as_errors(kwargs)
    features = cc_param_file_features(kwargs, srcs, hdrs)
    return dx_append_flags(kwargs, "features", features) if features else kwargs

def cc_library(name, srcs = None, hdrs = None, visibility = None, **kwargs):
    """Minimal wrapper over cc_library. On macOS it builds no dylib unless linkstatic is set."""
    kwargs = _cc_kwargs(kwargs, srcs, hdrs)
    if "linkstatic" not in kwargs:
        kwargs["linkstatic"] = select({
            Label("@platforms//os:macos"): True,
            "//conditions:default": False,
        })
    effective_srcs = srcs if srcs != None else []
    effective_hdrs = hdrs if hdrs != None else []
    dx_wrap(name, _cc_library, _cc_library_forward, effective_srcs, visibility = visibility, hdrs = effective_hdrs, extra_forward_kwargs = {"hdrs": effective_hdrs}, **kwargs)

def objc_library(name, visibility = None, **kwargs):
    """The upstream objc_library under the caller's name, so rules_apple and rules_swift see it unchanged."""
    _objc_library(name = name, visibility = visibility, **kwargs)

def cc_binary(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over cc_binary that links Linux binaries with a build-id."""
    kwargs = _cc_kwargs(kwargs, srcs)
    linkopts = kwargs.pop("linkopts", None) or []
    build_id = select({
        Label("@platforms//os:linux"): ["-Wl,--build-id=sha1"],
        "//conditions:default": [],
    })
    dx_wrap_binary(name, _cc_binary, _cc_binary_forward, srcs, visibility = visibility, linkopts = build_id + linkopts, **kwargs)

def cc_test(name, srcs = None, visibility = None, **kwargs):
    """Minimal wrapper over cc_test. On macOS (#8114) and Windows (matches upstream rules_cc) it links statically unless linkstatic is set. Under a GCC toolchain it sets ASAN_OPTIONS=verify_asan_link_order=0, which the caller's env overrides."""
    kwargs = _cc_kwargs(kwargs, srcs)
    caller_env = kwargs.pop("env", None) or {}
    gcc_env = select({
        Label("@rules_cc//cc/compiler:gcc"): {"ASAN_OPTIONS": "verify_asan_link_order=0"},
        "//conditions:default": {},
    })
    if "linkstatic" not in kwargs:
        kwargs["linkstatic"] = select({
            Label("@platforms//os:macos"): True,
            Label("@platforms//os:windows"): True,
            "//conditions:default": False,
        })
    dx_wrap_test(name, _cc_test, _cc_forward_test, srcs, visibility = visibility, env = gcc_env | caller_env, **kwargs)

def _cc_fuzz_transition_impl(_settings, _attr):
    return {
        "@llvm//config:asan": True,
        "@llvm//config:fuzzer": True,
    }

_cc_fuzz_transition = transition(
    implementation = _cc_fuzz_transition_impl,
    inputs = [],
    outputs = ["@llvm//config:asan", "@llvm//config:fuzzer"],
)

def _cc_fuzz_binary_impl(ctx):
    binary = ctx.attr.binary
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = binary[DefaultInfo].files_to_run.executable, is_executable = True)
    runfiles = ctx.runfiles(files = [executable]).merge(binary[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = executable, runfiles = runfiles)]

_cc_fuzz_binary = rule(
    implementation = _cc_fuzz_binary_impl,
    attrs = {
        "binary": attr.label(doc = "Binary the runner executes.", executable = True, cfg = "target", mandatory = True),
    },
    cfg = _cc_fuzz_transition,
    executable = True,
    doc = "Builds `binary` with the pinned LLVM's AddressSanitizer and libFuzzer runtimes.",
)

def cc_fuzz_test(name, srcs = None, visibility = None, tags = None, copts = None, linkopts = None, **kwargs):
    """A libFuzzer binary with AddressSanitizer plus a `<name>_run` runner for `dx fuzz`."""
    flags = ["-fsanitize=fuzzer,address"]
    cc_binary(
        name = name + "_binary",
        srcs = srcs,
        visibility = ["//visibility:private"],
        testonly = True,
        tags = ["manual"],
        copts = flags + (copts or []),
        linkopts = flags + (linkopts or []),
        **kwargs
    )
    _cc_fuzz_binary(
        name = name,
        binary = ":" + name + "_binary",
        visibility = visibility,
        testonly = True,
        tags = (tags or []) + ["dx-fuzz", "manual"],
    )
    sh_binary(
        name = name + "_run",
        srcs = [Label("//tools/fuzz:fuzz_run.sh")],
        args = ["$(rootpath :" + name + ")"],
        data = [":" + name],
        testonly = True,
        visibility = visibility,
    )
