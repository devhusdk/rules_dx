"""Links dx under the musl GHC tool names so the tools run under the bindist's own musl loader."""

def _musl_tools_impl(ctx):
    links = []
    for name in ctx.attr.tools:
        link = ctx.actions.declare_file(ctx.label.name + "/" + name)
        ctx.actions.symlink(output = link, target_file = ctx.executable._dx, is_executable = True)
        links.append(link)
    return [
        DefaultInfo(files = depset(links + [ctx.executable._dx] + ctx.files.srcs)),
        OutputGroupInfo(ghc = depset([link for link in links if link.basename == "ghc"])),
    ]

musl_tools = rule(
    implementation = _musl_tools_impl,
    doc = "Links dx under each musl GHC tool name, ahead of the bindist files.",
    attrs = {
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Files the linked tools run: binaries, config and loader closure."),
        "tools": attr.string_list(mandatory = True, doc = "Tool names to link."),
        "_dx": attr.label(default = Label("@dx_prebuilt//:dx"), executable = True, cfg = "exec"),
    },
)
