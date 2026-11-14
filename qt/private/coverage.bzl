"""LLVM coverage for qt_cc_test."""

load("//core:wrapper.bzl", "DxDataCoverageInfo", "dx_collect_cc_coverage_attr")

_CC_TOOLCHAIN_TYPE = "@bazel_tools//tools/cpp:toolchain_type"
_LLVM_COMPILERS = ["clang", "clang-cl"]
_OTHER_COMPILERS = ["gcc", "mingw-gcc", "msvc-cl"]
_LLVM_FEATURES = ["llvm_coverage_map_format", "-gcc_coverage_map_format"]
_COMPILER_FLAG = str(Label("//cc:compiler"))

def _features(features, collect_code_coverage, compiler):
    if not collect_code_coverage:
        return features
    if "-coverage" in features or "-llvm_coverage_map_format" in features:
        return features
    if compiler and compiler not in _LLVM_COMPILERS:
        return features
    return features + [f for f in _LLVM_FEATURES if f not in features]

def _transition_impl(settings, _attr):
    compiler = "gcc" if settings[_COMPILER_FLAG] == "gcc" else settings["//command_line_option:compiler"] or ""
    return {"//command_line_option:features": _features(settings["//command_line_option:features"], settings["//command_line_option:collect_code_coverage"], compiler)}

QT_COVERAGE_TRANSITION = transition(
    implementation = _transition_impl,
    inputs = ["//command_line_option:features", "//command_line_option:collect_code_coverage", "//command_line_option:compiler", _COMPILER_FLAG],
    outputs = ["//command_line_option:features"],
)

QtCoverageProbeInfo = provider(doc = "Whether LLVM coverage maps are enabled in the qt_cc_test configuration.", fields = {"llvm": "True when llvm_coverage_map_format is a feature."})

def _probe_impl(ctx):
    return [QtCoverageProbeInfo(llvm = "llvm_coverage_map_format" in ctx.features)]

qt_coverage_probe = rule(implementation = _probe_impl, doc = "Reports whether LLVM coverage maps are enabled in the test configuration.")

def qt_coverage_failure(compiler, llvm_maps):
    """Returns why qt_cc_test cannot collect coverage under compiler, or an empty string."""
    if compiler == "msvc-cl":
        return "qt_cc_test coverage needs an LLVM compiler (clang or clang-cl); the toolchain compiler is '%s'. Run without --collect_code_coverage or select an LLVM toolchain." % compiler
    if llvm_maps and compiler in _OTHER_COMPILERS:
        return "qt_cc_test coverage under the '%s' toolchain needs gcov coverage maps, but LLVM coverage features were added. Pass --compiler=%s." % (compiler, compiler)
    return ""

def qt_coverage_toolchains():
    """Returns the toolchains coverage needs to find llvm tools in the cc toolchain."""
    return [config_common.toolchain_type(_CC_TOOLCHAIN_TYPE, mandatory = False)]

def qt_coverage_attrs():
    """Returns the attributes LLVM coverage collection needs."""
    return dx_collect_cc_coverage_attr(Label("@dx_prebuilt//:collect_native")) | {
        "_macos": attr.label(default = Label("@platforms//os:macos")),
        "_llvm_cov": attr.label(default = Label("@llvm//tools:llvm-cov"), executable = True, allow_files = True, cfg = config.exec(exec_group = "test")),
        "_llvm_profdata": attr.label(default = Label("@llvm//tools:llvm-profdata"), executable = True, allow_files = True, cfg = config.exec(exec_group = "test")),
    }

def _cc_toolchain(ctx):
    info = ctx.toolchains[_CC_TOOLCHAIN_TYPE] if _CC_TOOLCHAIN_TYPE in ctx.toolchains else None
    if info == None:
        return None
    return info.cc if hasattr(info, "cc") else info

def _tool(cc_toolchain, basename):
    if cc_toolchain == None:
        return None
    for f in cc_toolchain.all_files.to_list():
        if f.basename == basename or f.basename == basename + ".exe":
            return f
    return None

def qt_test_coverage(ctx, upstream):
    """Returns the instrumented files and environment that collect LLVM coverage for a qt_cc_test."""
    collector = ctx.executable._collect_cc_coverage
    cc_toolchain = _cc_toolchain(ctx)
    macos = ctx.target_platform_has_constraint(ctx.attr._macos[platform_common.ConstraintValueInfo])
    llvm_cov = (None if macos else _tool(cc_toolchain, "llvm-cov")) or ctx.executable._llvm_cov
    llvm_profdata = (None if macos else _tool(cc_toolchain, "llvm-profdata")) or ctx.executable._llvm_profdata
    transitive = []
    if DxDataCoverageInfo in upstream:
        data = upstream[DxDataCoverageInfo]
        transitive = [data.metadata_files, data.instrumented_files]
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        dependency_attributes = ["upstream"],
        metadata_files = depset([collector, llvm_cov, llvm_profdata], transitive = transitive).to_list(),
    )
    env = {"CC_CODE_COVERAGE_SCRIPT": collector.path, "LLVM_COV": llvm_cov.path, "LLVM_PROFDATA": llvm_profdata.path}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env = dict(upstream[RunEnvironmentInfo].environment, **env)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    if DxDataCoverageInfo not in upstream:
        env["DX_COVERAGE_CLANG_OBJECTS"] = "test_binary"
    return struct(instrumented = instrumented, env = RunEnvironmentInfo(environment = env, inherited_environment = inherited))
