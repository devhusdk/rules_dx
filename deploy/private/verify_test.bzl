"""Runs the shared deploy verify harness in one mode."""

load("//core:wrapper.bzl", "dx_symlink_executable_name")

def _deploy_verify_test_impl(ctx):
    harness = ctx.executable.harness
    out = ctx.actions.declare_file(dx_symlink_executable_name(ctx.label.name, harness))
    ctx.actions.symlink(output = out, target_file = harness, is_executable = True)
    env = {key: ctx.expand_location(value, ctx.attr.data) for key, value in ctx.attr.env.items()}
    env["DX_VERIFY_MODE"] = ctx.attr.mode

    runfiles = ctx.runfiles(files = [out] + ctx.files.data).merge_all([d[DefaultInfo].default_runfiles for d in ctx.attr.data])
    return [
        DefaultInfo(executable = out, runfiles = runfiles),
        RunEnvironmentInfo(environment = env),
    ]

deploy_verify_test = rule(
    implementation = _deploy_verify_test_impl,
    doc = "Runs the deploy verify harness in one DX_VERIFY_MODE with its own data and env.",
    test = True,
    attrs = {
        "data": attr.label_list(allow_files = True, doc = "Files and targets the mode reads."),
        "env": attr.string_dict(doc = "Environment for the mode; values expand $(rootpath) against data."),
        "harness": attr.label(
            mandatory = True,
            executable = True,
            cfg = "target",
            doc = "The rust_test built from verify_test.rs.",
        ),
        "mode": attr.string(mandatory = True, doc = "The DX_VERIFY_MODE case to run."),
    },
)
