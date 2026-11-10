"""Placeholder executable target for analysis-only fixtures."""

def _stub_target_impl(ctx):
    exe = ctx.actions.declare_file(ctx.attr.out or ctx.label.name)
    ctx.actions.write(exe, ctx.attr.content, is_executable = True)
    return [DefaultInfo(executable = exe, files = depset([exe]), runfiles = ctx.runfiles(files = ctx.files.data))]

stub_target = rule(
    implementation = _stub_target_impl,
    doc = "Declares a placeholder executable for analysis-only fixtures.",
    attrs = {
        "content": attr.string(doc = "Executable file content."),
        "data": attr.label_list(allow_files = True, doc = "Files added to the runfiles."),
        "out": attr.string(doc = "Output file name; defaults to the target name."),
    },
    executable = True,
)
