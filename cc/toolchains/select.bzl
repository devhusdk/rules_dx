"""Run a test under an explicitly selected C++ toolchain."""

def _impl(settings, attr):
    if not attr.extra_toolchain:
        return {
            "//command_line_option:collect_code_coverage": settings["//command_line_option:collect_code_coverage"],
            "//command_line_option:extra_toolchains": settings["//command_line_option:extra_toolchains"],
        }
    return {
        "//command_line_option:collect_code_coverage": False,
        "//command_line_option:extra_toolchains": [attr.extra_toolchain] + settings["//command_line_option:extra_toolchains"],
    }

toolchain_transition = transition(
    implementation = _impl,
    inputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:extra_toolchains",
    ],
    outputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:extra_toolchains",
    ],
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
    return providers

cc_toolchain_test = rule(
    implementation = _test_impl,
    doc = "Runs a test built with the selected C++ toolchain.",
    attrs = {
        "extra_toolchain": attr.string(doc = "Toolchain label to prefer.", mandatory = True),
        "test": attr.label(doc = "Test to rebuild.", executable = True, cfg = toolchain_transition, mandatory = True),
    },
    test = True,
)
