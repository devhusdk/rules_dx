"""Exposes the resolved Dart SDK's `dart` binary as an executable target."""

def _dart_tool_impl(ctx):
    sdk = ctx.toolchains["@rules_dart//dart:toolchain_type"].dart_sdk_info
    root = sdk.dart.path.rsplit("/", 2)[0] + "/"
    exe = None
    mirrored = []
    aot_runtime = False
    core_lib = False
    for f in sdk.tool_files.to_list():
        # dart finds its snapshots beside its resolved path, and a remote executor
        # materializes a symlinked executable away from the files around it. So the
        # executable, its snapshots, `dartaotruntime` and `lib/` (which `dart analyze`
        # needs to resolve dart:core) are mirrored together under one tree.
        if not f.path.startswith(root):
            continue
        rel = f.path[len(root):]
        is_exe = f.path == sdk.dart.path
        aot = rel in ["bin/dartaotruntime", "bin/dartaotruntime.exe"]
        if not (is_exe or aot or rel.startswith("bin/snapshots/") or rel.startswith("lib/") or rel == "version"):
            continue
        link = ctx.actions.declare_file(ctx.label.name + "/sdk/" + rel)
        ctx.actions.symlink(output = link, target_file = f, is_executable = is_exe or aot)
        mirrored.append(link)
        if is_exe:
            exe = link
        aot_runtime = aot_runtime or aot
        core_lib = core_lib or rel.startswith("lib/core/")
    if exe == None:
        fail("the Dart SDK toolchain has no %s among its tool files" % sdk.dart.path)
    if not aot_runtime or not core_lib:
        fail("the Dart SDK toolchain lacks bin/dartaotruntime or lib/core/, which dart analyze needs")
    return [DefaultInfo(
        executable = exe,
        files = depset([exe]),
        runfiles = ctx.runfiles(files = mirrored),
    )]

dart_tool = rule(
    implementation = _dart_tool_impl,
    executable = True,
    toolchains = ["@rules_dart//dart:toolchain_type"],
    doc = "The `dart` binary and its snapshots, from the resolved rules_dart toolchain.",
)
