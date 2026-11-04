"""Run a test under an explicitly selected C++ toolchain."""

load("//libs/starlark:wrapper.bzl", "dx_lcov_merger_attr")

_COMPILER = str(Label("//cc/toolchains:compiler"))

def _impl(settings, attr):
    return {_COMPILER: attr.compiler or settings[_COMPILER]}

toolchain_transition = transition(
    implementation = _impl,
    inputs = [_COMPILER],
    outputs = [_COMPILER],
)

def _test_impl(ctx):
    src = ctx.attr.test[0]
    inner = src[DefaultInfo].files_to_run.executable
    path = inner.short_path
    path = path[3:] if path.startswith("../") else ctx.workspace_name + "/" + path
    exe = ctx.actions.declare_file(ctx.label.name + ".sh")
    ctx.actions.write(
        output = exe,
        content = "#!/bin/bash\nexec \"${RUNFILES_DIR:-$0.runfiles}/%s\" \"$@\"\n" % path,
        is_executable = True,
    )
    runfiles = ctx.runfiles(files = [inner]).merge(src[DefaultInfo].default_runfiles)
    providers = [DefaultInfo(executable = exe, runfiles = runfiles)]
    if RunEnvironmentInfo in src:
        providers.append(src[RunEnvironmentInfo])
    providers.append(coverage_common.instrumented_files_info(ctx, dependency_attributes = ["test"]))
    return providers

cc_toolchain_test = rule(
    implementation = _test_impl,
    doc = "Runs a test built with the selected C++ toolchain.",
    attrs = {
        "compiler": attr.string(doc = "Compiler flag value to build with.", mandatory = True),
        "test": attr.label(doc = "Test to rebuild.", executable = True, cfg = toolchain_transition, mandatory = True),
    } | dx_lcov_merger_attr(),
    test = True,
)
