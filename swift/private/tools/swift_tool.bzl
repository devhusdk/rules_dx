"""Exposes swift-format from the pinned Swift toolchain archive."""

def _swift_format_impl(ctx):
    exe = None
    libs = []
    for f in ctx.files.srcs:
        if f.path.endswith("/usr/bin/swift-format"):
            exe = f
        elif "/usr/lib/swift/linux/" in f.path and f.path.endswith(".so"):
            libs.append(f)
    if exe == None:
        fail("the Swift toolchain archive has no usr/bin/swift-format")
    copy = ctx.actions.declare_file(ctx.label.name + "/usr/bin/swift-format")
    # run_shell: A copy, not a symlink, so the binary finds its libraries next to its real path.
    ctx.actions.run_shell(
        inputs = [exe],
        outputs = [copy],
        command = "cp -L \"$1\" \"$2\" && chmod +x \"$2\"",
        arguments = [exe.path, copy.path],
        mnemonic = "SwiftFormatCopy",
    )
    mirrored = [copy]
    for lib in libs:
        link = ctx.actions.declare_file(ctx.label.name + "/usr/lib/swift/linux/" + lib.basename)
        ctx.actions.symlink(output = link, target_file = lib)
        mirrored.append(link)
    return [DefaultInfo(
        executable = copy,
        files = depset([copy]),
        runfiles = ctx.runfiles(files = mirrored),
    )]

swift_format_tool = rule(
    implementation = _swift_format_impl,
    attrs = {"srcs": attr.label_list(doc = "Swift sources to check.", allow_files = True, mandatory = True)},
    executable = True,
    doc = "A copy of swift-format beside links to the libraries it loads.",
)
