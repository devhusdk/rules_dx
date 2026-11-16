"""Bundles the lsp_check driver with a JDK and a language server jar."""

load("@rules_java//java/common:java_common.bzl", "java_common")

PklLspToolInfo = provider(
    doc = "The java launcher and the server jar of a bundled JVM language server.",
    fields = {
        "jar": "File: the language server jar.",
        "java": "File: the java launcher inside the pinned JDK.",
        "natives": "File: a jar of native libraries the server loads as classpath resources.",
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
    natives = ctx.actions.declare_file(ctx.label.name + "/natives.jar")
    marker = ctx.actions.declare_file(ctx.label.name + "/natives.marker")
    ctx.actions.write(marker, "")
    zipper = ctx.actions.args()
    zipper.add("c", natives)
    zipper.add("META-INF/dx-natives=" + marker.path)
    for lib in ctx.files.natives:
        zipper.add(ctx.attr.native_prefix + "/" + lib.basename + "=" + lib.path)
    ctx.actions.run(
        executable = ctx.executable._zipper,
        arguments = [zipper],
        inputs = [marker] + ctx.files.natives,
        outputs = [natives],
        mnemonic = "JarLspNatives",
    )
    runfiles = ctx.runfiles(files = [ctx.file.jar, natives], transitive_files = runtime.files)
    runfiles = runfiles.merge(ctx.attr.driver[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = exe, runfiles = runfiles),
        OutputGroupInfo(java = depset([java]), jar = depset([ctx.file.jar]), natives = depset([natives])),
        PklLspToolInfo(java = java, jar = ctx.file.jar, natives = natives),
    ]

jar_lsp_tool = rule(
    implementation = _impl,
    doc = "Runs `driver` with a pinned JDK and a language server jar in its runfiles.",
    attrs = {
        "driver": attr.label(doc = "Executable that starts the language server.", mandatory = True, executable = True, cfg = "exec"),
        "jar": attr.label(doc = "Language server jar.", mandatory = True, allow_single_file = True),
        "native_prefix": attr.string(doc = "Resource directory of `natives` inside the natives jar."),
        "natives": attr.label_list(doc = "Native libraries packed into the natives jar under `native_prefix`.", allow_files = True),
        "runtime": attr.label(doc = "Java runtime that runs the jar.", mandatory = True, cfg = "exec", providers = [java_common.JavaRuntimeInfo]),
        "_zipper": attr.label(default = Label("@bazel_tools//tools/zip:zipper"), executable = True, cfg = "exec"),
    },
    executable = True,
)
