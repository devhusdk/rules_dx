"""Repository-root candidates for dx codegen and dx env."""

def _repository_roots_file_impl(ctx):
    content = "\n".join(ctx.attr.roots)
    if content:
        content += "\n"
    ctx.actions.write(output = ctx.outputs.out, content = content)

repository_roots_file = rule(
    implementation = _repository_roots_file_impl,
    doc = "Writes repository-root candidates to a text file, one per line.",
    attrs = {
        "roots": attr.string_list(
            doc = "Repository-root candidates written one per line.",
            mandatory = True,
        ),
        "out": attr.output(
            doc = "Output text file that receives the root candidates.",
            mandatory = True,
        ),
    },
)

def roots_aggregate(name, deps, **kwargs):
    """One aggregate root: a plain filegroup over deps."""
    native.filegroup(name = name, srcs = deps, **kwargs)
