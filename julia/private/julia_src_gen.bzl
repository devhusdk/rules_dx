"""Generates the Julia runtime sources the upstream Makefile makes with the shell."""

visibility("public")

def _julia_src_gen_impl(ctx):
    targets = ctx.attr.srcs + ctx.attr.tools
    args = ctx.actions.args()
    args.add(ctx.attr.mode)
    args.add_all([ctx.expand_location(arg, targets) for arg in ctx.attr.arguments])
    args.add(ctx.outputs.out)
    ctx.actions.run(
        executable = ctx.executable._gen,
        arguments = [args],
        inputs = ctx.files.srcs,
        tools = [tool[DefaultInfo].files_to_run for tool in ctx.attr.tools],
        outputs = [ctx.outputs.out],
        mnemonic = "JuliaSrcGen",
        progress_message = "Generating %{output}",
    )
    return [DefaultInfo(files = depset([ctx.outputs.out]))]

julia_src_gen = rule(
    implementation = _julia_src_gen_impl,
    doc = "Runs one julia_src_gen mode as `julia_src_gen MODE ARG... OUT`.",
    attrs = {
        "arguments": attr.string_list(doc = "Arguments after the mode, with $(execpath) expanded against srcs and tools."),
        "mode": attr.string(doc = "The julia_src_gen mode.", mandatory = True, values = ["version-h", "internal-funcs", "data-globals", "expmap", "flisp-boot", "flisp-filter"]),
        "out": attr.output(doc = "The generated file.", mandatory = True),
        "srcs": attr.label_list(doc = "Files the mode reads.", allow_files = True),
        "tools": attr.label_list(doc = "Executables the mode runs.", allow_files = True, cfg = "exec"),
        "_gen": attr.label(default = Label("//julia/private/tools:julia_src_gen"), executable = True, cfg = "exec"),
    },
)
