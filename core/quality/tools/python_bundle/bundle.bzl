"""Builds the relocatable archive of one core Python quality tool for one release platform."""

def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".tar.gz")
    args = ctx.actions.args()
    args.add(out)
    args.add(ctx.executable._uv)
    args.add(ctx.file.python_marker)
    args.add(ctx.attr.uv_platform)
    args.add("true" if ctx.attr.windows else "false")
    args.add(ctx.file.tool_bootstrap)
    args.add(ctx.file.project.dirname)
    args.add(ctx.attr.package)
    args.add(ctx.attr.launcher)
    args.add(ctx.file.tool_main)
    ctx.actions.run(
        executable = ctx.executable._script,
        arguments = [args],
        inputs = depset(
            ctx.files.python_files + [ctx.file.lock_file, ctx.file.project, ctx.file.python_marker, ctx.file.tool_bootstrap, ctx.file.tool_main],
        ),
        tools = [ctx.executable._uv, ctx.file._interpreter],
        outputs = [out],
        env = {"PYTHON_EXEC": ctx.file._interpreter.path},
        execution_requirements = {"requires-network": "1", "no-sandbox": "1"},
        mnemonic = "PythonToolBundle",
        progress_message = "Bundling %{label}",
    )
    return [DefaultInfo(files = depset([out]))]

python_tool_bundle = rule(
    implementation = _impl,
    attrs = {
        "launcher": attr.string(mandatory = True, doc = "The bin/ launcher name, such as yamllint."),
        "lock_file": attr.label(allow_single_file = True, mandatory = True, doc = "The uv.lock beside project."),
        "package": attr.string(mandatory = True, doc = "The locked PyPI package the tool runs; the archive carries only its dependency closure."),
        "project": attr.label(allow_single_file = True, mandatory = True, doc = "The pyproject.toml whose lock pins the tool."),
        "python_files": attr.label(mandatory = True),
        "python_marker": attr.label(allow_single_file = True, mandatory = True),
        "tool_bootstrap": attr.label(allow_single_file = True, mandatory = True),
        "tool_main": attr.label(allow_single_file = True, mandatory = True, doc = "The <launcher>_main.py the launcher runs."),
        "uv_platform": attr.string(mandatory = True),
        "windows": attr.bool(),
        "_interpreter": attr.label(default = "@python_3_12//:python3", allow_single_file = True, cfg = "exec"),
        "_script": attr.label(default = ":build_bundle", executable = True, cfg = "exec"),
        "_uv": attr.label(default = "@dx_tools//:uv", executable = True, cfg = "exec"),
    },
)
