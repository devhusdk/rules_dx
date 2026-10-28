"""Exposes the resolved Dart SDK's `dart` binary as an executable target."""

def _dart_tool_impl(ctx):
    sdk = ctx.toolchains["@rules_dart//dart:toolchain_type"].dart_sdk_info
    root = sdk.dart.path.rsplit("/", 2)[0] + "/"
    bin_dir = root + "bin/"
    exe = None
    mirrored = []
    for f in sdk.tool_files.to_list():
        # dart finds its snapshots beside its resolved path, and a remote executor
        # materializes a symlinked executable away from the files around it. So the
        # executable and its snapshots are mirrored together under one tree.
        if not f.path.startswith(bin_dir) or not (f.path == sdk.dart.path or f.path.startswith(bin_dir + "snapshots/")):
            continue
        link = ctx.actions.declare_file(ctx.label.name + "/sdk/" + f.path[len(root):])
        ctx.actions.symlink(output = link, target_file = f, is_executable = f.path == sdk.dart.path)
        mirrored.append(link)
        if f.path == sdk.dart.path:
            exe = link
    if exe == None:
        fail("the Dart SDK toolchain has no %s among its tool files" % sdk.dart.path)
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
