"""Builds the relocatable bundle of the core Python quality tools for one release platform."""

def _impl(ctx):
    out = ctx.actions.declare_file(ctx.label.name + ".tar.gz")
    args = ctx.actions.args()
    args.add(out)
    args.add(ctx.executable._uv)
    args.add(ctx.file.python_marker)
    args.add(ctx.attr.uv_platform)
    args.add("true" if ctx.attr.windows else "false")
    args.add(" ".join(ctx.attr.skip_packages))
    args.add(ctx.file.tool_bootstrap.dirname)
    for project, spec in ctx.attr.projects.items():
        name, _, tools = spec.partition(":")
        args.add("{}={}={}".format(name, project.files.to_list()[0].dirname, tools))
    ctx.actions.run(
        executable = ctx.executable._script,
        arguments = [args],
        inputs = depset(
            ctx.files.python_files + ctx.files.tool_sources + ctx.files.lock_files + [ctx.file.python_marker, ctx.file.tool_bootstrap],
        ),
        tools = [ctx.executable._uv, ctx.file._interpreter],
        outputs = [out],
        env = {"PYTHON_EXEC": ctx.file._interpreter.path},
        execution_requirements = {"requires-network": "1", "no-sandbox": "1"},
        mnemonic = "PythonToolsBundle",
        progress_message = "Bundling the Python quality tools for %{label}",
    )
    return [DefaultInfo(files = depset([out]))]

python_tools_bundle = rule(
    implementation = _impl,
    attrs = {
        "lock_files": attr.label_list(allow_files = True),
        "projects": attr.label_keyed_string_dict(allow_files = True, doc = "pyproject.toml to 'group:tool,tool'."),
        "python_files": attr.label(mandatory = True),
        "python_marker": attr.label(allow_single_file = True, mandatory = True),
        "skip_packages": attr.string_list(),
        "tool_bootstrap": attr.label(allow_single_file = True, mandatory = True),
        "tool_sources": attr.label_list(allow_files = True),
        "uv_platform": attr.string(mandatory = True),
        "windows": attr.bool(),
        "_interpreter": attr.label(default = "@python_3_12//:python3", allow_single_file = True, cfg = "exec"),
        "_script": attr.label(default = ":build_bundle", executable = True, cfg = "exec"),
        "_uv": attr.label(default = "@dx_tools//:uv", executable = True, cfg = "exec"),
    },
)
