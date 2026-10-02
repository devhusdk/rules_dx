"""Repository-root candidates for dx codegen, dx env, and dx setup."""

def _repository_roots_file_impl(ctx):
    content = "\n".join(ctx.attr.roots)
    if content:
        content += "\n"
    ctx.actions.write(output = ctx.outputs.out, content = content)

repository_roots_file = rule(
    implementation = _repository_roots_file_impl,
    attrs = {
        "roots": attr.string_list(
            mandatory = True,
        ),
        "out": attr.output(
            mandatory = True,
        ),
    },
)

def roots_aggregate(name, deps, **kwargs):
    """One aggregate root: a plain filegroup over deps."""
    native.filegroup(name = name, srcs = deps, **kwargs)
