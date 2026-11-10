"""Bundles the lsp_check driver with a JDK and a language server jar."""

load("@rules_java//java/common:java_common.bzl", "java_common")

PklLspToolInfo = provider(
    doc = "The java launcher and the server jar of a bundled JVM language server.",
    fields = {
        "jar": "File: the language server jar.",
        "java": "File: the java launcher inside the pinned JDK.",
    },
)

def _impl(ctx):
    runtime = ctx.attr.runtime[java_common.JavaRuntimeInfo]
    java = None
    for f in runtime.files.to_list():
        if f.path == runtime.java_executable_exec_path:
            java = f
    if java == None:
        fail(ctx.label.name + ": the JDK has no file at " + runtime.java_executable_exec_path)
    driver = ctx.executable.driver
    extension = driver.extension
    exe = ctx.actions.declare_file(ctx.label.name + "/lsp_check" + (("." + extension) if extension else ""))
    ctx.actions.symlink(output = exe, target_file = driver, is_executable = True)
    runfiles = ctx.runfiles(files = [ctx.file.jar], transitive_files = runtime.files)
    runfiles = runfiles.merge(ctx.attr.driver[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = exe, runfiles = runfiles),
        OutputGroupInfo(java = depset([java]), jar = depset([ctx.file.jar])),
        PklLspToolInfo(java = java, jar = ctx.file.jar),
    ]

jar_lsp_tool = rule(
    implementation = _impl,
    doc = "Runs `driver` with a pinned JDK and a language server jar in its runfiles.",
    attrs = {
        "driver": attr.label(doc = "Executable that starts the language server.", mandatory = True, executable = True, cfg = "exec"),
        "jar": attr.label(doc = "Language server jar.", mandatory = True, allow_single_file = True),
        "runtime": attr.label(doc = "Java runtime that runs the jar.", mandatory = True, cfg = "exec", providers = [java_common.JavaRuntimeInfo]),
    },
    executable = True,
)
