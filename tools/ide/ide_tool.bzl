"""Rules that expose the toolchain-resolved tool of one SDK as a single file.

Each rule returns the original tool file (not a symlink), because Java, Go and
.NET find their home directory from the real path of their own binary. The rest
of the SDK travels as runfiles. One rule per toolchain type means a test only
fetches its own SDK.
"""

def _tool_info(ctx, tool, runfiles_files = None, runfiles_transitive = None):
    if tool == None:
        fail("%s: the resolved toolchain has no tool file" % ctx.label)
    return [DefaultInfo(
        files = depset([tool]),
        runfiles = ctx.runfiles(
            files = [tool] + (runfiles_files or []),
            transitive_files = runfiles_transitive,
        ),
    )]

def _go_impl(ctx):
    sdk = ctx.toolchains["@rules_go//go:toolchain"].sdk
    return _tool_info(
        ctx,
        sdk.go,
        [sdk.root_file],
        depset(transitive = [sdk.tools, sdk.libs]),
    )

ide_go_tool = rule(
    implementation = _go_impl,
    toolchains = ["@rules_go//go:toolchain"],
    doc = "The go binary from the resolved rules_go toolchain.",
)

def _node_impl(ctx):
    info = ctx.toolchains["@rules_nodejs//nodejs:runtime_toolchain_type"].nodeinfo
    return _tool_info(ctx, info.node, info.tool_files)

ide_node_tool = rule(
    implementation = _node_impl,
    toolchains = ["@rules_nodejs//nodejs:runtime_toolchain_type"],
    doc = "The node binary from the resolved rules_nodejs toolchain.",
)

def _java_impl(ctx):
    runtime = ctx.toolchains["@bazel_tools//tools/jdk:runtime_toolchain_type"].java_runtime
    exe = runtime.java_executable_exec_path
    matches = [f for f in runtime.files.to_list() if f.path == exe]
    if len(matches) != 1:
        fail("%s: want one file at %s in the Java runtime, got %d" % (ctx.label, exe, len(matches)))
    return _tool_info(ctx, matches[0], None, runtime.files)

ide_java_tool = rule(
    implementation = _java_impl,
    toolchains = ["@bazel_tools//tools/jdk:runtime_toolchain_type"],
    doc = "The java binary from the resolved Java runtime toolchain.",
)

def _python_impl(ctx):
    runtime = ctx.toolchains["@bazel_tools//tools/python:toolchain_type"].py3_runtime
    return _tool_info(ctx, runtime.interpreter, None, runtime.files)

ide_python_tool = rule(
    implementation = _python_impl,
    toolchains = ["@bazel_tools//tools/python:toolchain_type"],
    doc = "The Python 3 interpreter from the resolved Python toolchain.",
)

def _dotnet_impl(ctx):
    runtime = ctx.toolchains["@rules_dotnet//dotnet:toolchain_type"].runtime
    return _tool_info(
        ctx,
        runtime.files_to_run.executable,
        None,
        runtime.default_runfiles.files,
    )

ide_dotnet_tool = rule(
    implementation = _dotnet_impl,
    toolchains = ["@rules_dotnet//dotnet:toolchain_type"],
    doc = "The dotnet host from the resolved rules_dotnet toolchain.",
)

def _rust_analyzer_impl(ctx):
    tool = ctx.toolchains["@rules_rust//rust/rust_analyzer:toolchain_type"].rust_analyzer
    return _tool_info(ctx, tool)

ide_rust_analyzer_tool = rule(
    implementation = _rust_analyzer_impl,
    toolchains = ["@rules_rust//rust/rust_analyzer:toolchain_type"],
    doc = "The rust-analyzer binary from the resolved rules_rust toolchain.",
)
