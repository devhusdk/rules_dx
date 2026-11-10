"""Native (C/C++/LLVM) coverage collection for wrapper rules."""

load("@rules_cc//cc:find_cc_toolchain.bzl", "find_cc_toolchain", "use_cc_toolchain")
load(":wrapper.bzl", "DxDataCoverageInfo", "dx_collect_cc_coverage_attr")

def dx_native_coverage_toolchains():
    """Returns the toolchains native coverage needs to find llvm tools in the cc toolchain."""
    return use_cc_toolchain(mandatory = False)

def dx_toolchain_tool(cc_toolchain, basename):
    """Returns the cc toolchain file named basename (with optional .exe), or None."""
    if cc_toolchain == None:
        return None
    for f in cc_toolchain.all_files.to_list():
        if f.basename == basename or f.basename == basename + ".exe":
            return f
    return None

def dx_native_coverage_attrs():
    """Returns the attributes native (C/C++/LLVM) coverage collection needs."""
    return dx_collect_cc_coverage_attr(Label("@dx_prebuilt//:collect_native")) | {
        "_llvm_cov": attr.label(
            default = Label("@llvm//tools:llvm-cov"),
            executable = True,
            allow_files = True,
            cfg = config.exec(exec_group = "test"),
        ),
        "_llvm_profdata": attr.label(
            default = Label("@llvm//tools:llvm-profdata"),
            executable = True,
            allow_files = True,
            cfg = config.exec(exec_group = "test"),
        ),
        "_cc_toolchain": attr.label(default = Label("@rules_cc//cc:current_cc_toolchain")),
    }

def dx_gcov_tool(cc_toolchain):
    """Returns the gcov File that ships with the cc toolchain, or fails."""
    gcov_path = cc_toolchain.gcov_executable
    for f in cc_toolchain.all_files.to_list():
        if f.path == gcov_path:
            return f
    fail("the gcc toolchain does not ship gcov at " + gcov_path)

def dx_native_test_coverage(ctx, upstream, gcov = False, llvm_cov = None, llvm_profdata = None):
    """Returns coverage providers that collect native LLVM or gcov coverage."""
    collector = ctx.executable._collect_cc_coverage
    cc_toolchain = find_cc_toolchain(ctx, mandatory = False)
    gcov_tool = None
    if gcov:
        gcov_tool = dx_gcov_tool(cc_toolchain)
        llvm_cov = None
        llvm_profdata = None
        metadata = [collector, gcov_tool]
    else:
        llvm_cov = llvm_cov or dx_toolchain_tool(cc_toolchain, "llvm-cov") or ctx.executable._llvm_cov
        llvm_profdata = llvm_profdata or dx_toolchain_tool(cc_toolchain, "llvm-profdata") or ctx.executable._llvm_profdata
        metadata = [collector, llvm_cov, llvm_profdata]
    transitive = []
    if DxDataCoverageInfo in upstream:
        data = upstream[DxDataCoverageInfo]
        transitive = [data.metadata_files, data.instrumented_files]
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        dependency_attributes = ["upstream"],
        metadata_files = depset(metadata, transitive = transitive).to_list(),
    )
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    env["CC_CODE_COVERAGE_SCRIPT"] = collector.path
    if gcov:
        env["GCOV"] = gcov_tool.path
    else:
        if DxDataCoverageInfo not in upstream:
            env["DX_COVERAGE_CLANG_OBJECTS"] = "test_binary"
        env["LLVM_COV"] = llvm_cov.path
        env["LLVM_PROFDATA"] = llvm_profdata.path
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = None,
    )
