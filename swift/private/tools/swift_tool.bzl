"""Exposes swift-format from the pinned Swift toolchain archive."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")
load("//core/coreutils:defs.bzl", "COREUTILS_TOOLCHAIN_TYPE", "coreutils")

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
    applets = coreutils(ctx)
    ctx.actions.run(
        executable = applets.binary,
        arguments = ["cp", "-L", exe.path, copy.path],
        inputs = [exe],
        outputs = [copy],
        env = {"PATH": "/dx_no_path"},
        mnemonic = "SwiftFormatCopy",
        use_default_shell_env = False,
        toolchain = COREUTILS_TOOLCHAIN_TYPE,
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
    toolchains = [COREUTILS_TOOLCHAIN_TYPE],
)

def _swift_format_windows_impl(ctx):
    program = ctx.file.program
    # swift-format.exe names the Swift runtime as side-by-side assemblies, which Windows looks up
    # in real directories beside the real executable. A runfiles tree holds them as symlinks and
    # the lookup fails there with os error 14001, so run the executable from where it really sits,
    # as swiftc runs.
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "real_program=" + rlocation_path(ctx, program) + "\n")
    runfiles = launcher_runfiles.merge(ctx.runfiles(files = [program], transitive_files = depset(ctx.files.data)))
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

swift_format_windows = rule(
    implementation = _swift_format_windows_impl,
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "data": attr.label_list(allow_files = True, doc = "The toolchain bin directory swift-format loads its DLLs from."),
        "program": attr.label(allow_single_file = True, mandatory = True, doc = "swift-format.exe inside the Swift Windows toolchain."),
    },
    executable = True,
    doc = "Runs swift-format.exe of the pinned Swift Windows toolchain from its real path, beside its DLLs and side-by-side assemblies.",
)
