"""The Linux `swift` command: a native launcher that runs the managed toolchain's `swift`."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")

def _swift_host_launcher_impl(ctx):
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "\n".join([
        "program=" + rlocation_path(ctx, ctx.file.swift),
        "env_dir=COMPILER_PATH\t" + rlocation_path(ctx, ctx.file.linker),
        "env_dir=LD_LIBRARY_PATH\t" + rlocation_path(ctx, ctx.file.libs_anchor),
    ]) + "\n")
    runfiles = ctx.runfiles(files = [ctx.file.swift, ctx.file.linker, ctx.file.libs_anchor] + ctx.files.data)
    return DefaultInfo(
        executable = launcher,
        runfiles = launcher_runfiles.merge(runfiles),
    )

swift_host_launcher = rule(
    implementation = _swift_host_launcher_impl,
    doc = "A launcher that runs the managed Linux `swift` with its linker and host libraries.",
    attrs = {
        "data": attr.label_list(allow_files = True, doc = "The toolchain files `swift` reads at run time."),
        "libs_anchor": attr.label(allow_single_file = True, mandatory = True, doc = "A file in the directory `LD_LIBRARY_PATH` names."),
        "linker": attr.label(allow_single_file = True, mandatory = True, doc = "The `ld` whose directory `COMPILER_PATH` names."),
        "swift": attr.label(allow_single_file = True, mandatory = True, doc = "The `swift` driver."),
    } | NATIVE_LAUNCHER_ATTRS,
    executable = True,
)
