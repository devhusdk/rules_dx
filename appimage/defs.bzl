"""Linux AppImage packaging for target binaries."""

load("//appimage:pins.bzl", "MKSQUASHFS_VERSION", "RUNTIME_VERSION", "TOOL_VERSION")
load("//libs/starlark:packaging.bzl", "claim_dest", "data_path", "platform_arch")

AppImageInfo = provider(
    doc = "Identity of one packaged AppImage.",
    fields = {
        "identity": "File: JSON with the packaged byte identities.",
        "image": "File: the AppImage.",
        "name": "String: application name.",
        "version": "String: application version.",
    },
)

def name_errors(name, desktop, icon):
    """Returns the metadata names that do not match the application name."""
    errors = []
    if desktop != name + ".desktop":
        errors.append("desktop file '" + desktop + "' must be named " + name + ".desktop")
    if icon not in [name + ".png", name + ".svg"]:
        errors.append("icon '" + icon + "' must be named " + name + ".png or " + name + ".svg")
    return errors

def _add(ctx, args, inputs, seen, kind, dest, arg, file):
    """Adds one packaged file once, failing when a different file maps to its destination."""
    prior = claim_dest(seen, dest, file)
    if prior != None and prior.path == file.path:
        return
    if prior != None:
        fail("dx_appimage " + str(ctx.label) + ": " + kind + " files " + prior.short_path + " and " + file.short_path + " both map to " + dest)
    args.add(arg)
    inputs.append(file)

def _appimage_impl(ctx):
    cv = platform_common.ConstraintValueInfo
    arch = platform_arch(ctx.target_platform_has_constraint, ctx.attr._linux[cv], [
        ("aarch64", ctx.attr._arm64[cv]),
        ("x86_64", ctx.attr._x86_64[cv]),
    ])
    if not arch:
        fail("dx_appimage " + str(ctx.label) + ": target cell this platform is unsupported: only linux_x86_64 and linux_arm64 are pinned")
    name = ctx.attr.app_name or ctx.label.name
    errors = name_errors(name, ctx.file.desktop.basename, ctx.file.icon.basename)
    if errors:
        fail("dx_appimage " + str(ctx.label) + ": " + "; ".join(errors))
    exe = ctx.attr.binary[DefaultInfo].files_to_run.executable
    if exe == None:
        fail("dx_appimage " + str(ctx.label) + ": binary " + str(ctx.attr.binary.label) + " has no executable")
    tool_files = ctx.attr.tools[DefaultInfo].files.to_list()
    appimagetool = [f for f in tool_files if f.basename == "appimagetool"]
    if len(appimagetool) != 1:
        fail("dx_appimage " + str(ctx.label) + ": tools must provide exactly one appimagetool")
    runtime = ctx.file.runtime or (ctx.file._runtime_aarch64 if arch == "aarch64" else ctx.file._runtime_x86_64)

    image = ctx.actions.declare_file(ctx.label.name + ".AppImage")
    identity = ctx.actions.declare_file(ctx.label.name + ".AppImage.identity.json")

    args = ctx.actions.args()
    args.add(appimagetool[0].dirname)
    args.add(runtime)
    args.add(name)
    args.add(ctx.attr.version)
    args.add(image)
    args.add(identity)
    args.add(exe)
    args.add(ctx.file.desktop)
    args.add(ctx.file.icon)
    args.add(arch)
    args.add(TOOL_VERSION)
    args.add(RUNTIME_VERSION)
    args.add(MKSQUASHFS_VERSION)
    inputs = [exe, ctx.file.desktop, ctx.file.icon, runtime] + tool_files
    seen = {}
    for target in ctx.attr.libs:
        for file in target[DefaultInfo].files.to_list():
            _add(ctx, args, inputs, seen, "lib", "usr/lib/" + file.basename, "lib:" + file.path, file)
    for target in ctx.attr.notice:
        for file in target[DefaultInfo].files.to_list():
            _add(ctx, args, inputs, seen, "notice", "usr/share/licenses/" + name + "/" + file.basename, "notice:" + file.path, file)
    for target in ctx.attr.data:
        for file in target[DefaultInfo].files.to_list():
            rel = data_path(file, target.label)
            _add(ctx, args, inputs, seen, "data", "usr/share/" + name + "/" + rel, "data:" + rel + ":" + file.path, file)

    ctx.actions.run(
        executable = ctx.executable._build,
        arguments = [args],
        inputs = inputs,
        outputs = [image, identity],
        mnemonic = "DxAppImage",
        progress_message = "Packaging AppImage " + str(ctx.label),
        use_default_shell_env = False,
        env = {"LC_ALL": "C", "SOURCE_DATE_EPOCH": "0"},
    )
    return [
        DefaultInfo(files = depset([image, identity])),
        AppImageInfo(identity = identity, image = image, name = name, version = ctx.attr.version),
    ]

dx_appimage = rule(
    implementation = _appimage_impl,
    doc = "Packages one target binary and its files as a Linux AppImage.",
    attrs = {
        "app_name": attr.string(doc = "Application name. Defaults to the target name."),
        "binary": attr.label(doc = "The executable to package.", mandatory = True, executable = True, cfg = "target"),
        "data": attr.label_list(doc = "Resources placed under usr/share/<name>.", allow_files = True),
        "desktop": attr.label(doc = "The <name>.desktop file.", mandatory = True, allow_single_file = [".desktop"]),
        "icon": attr.label(doc = "The <name>.png or <name>.svg icon.", mandatory = True, allow_single_file = [".png", ".svg"]),
        "libs": attr.label_list(doc = "Shared libraries placed under usr/lib.", allow_files = True),
        "notice": attr.label_list(doc = "License and NOTICE files placed under usr/share/licenses/<name>.", allow_files = True, mandatory = True, allow_empty = False),
        "runtime": attr.label(doc = "The type 2 runtime for the target arch. Defaults to the pinned one.", allow_single_file = True, cfg = "target"),
        "tools": attr.label(doc = "The appimagetool closure.", cfg = "exec", default = "@dx_appimage_tools//:tools"),
        "version": attr.string(doc = "Application version.", mandatory = True),
        "_arm64": attr.label(default = "@platforms//cpu:arm64"),
        "_build": attr.label(default = "//appimage/packer:appimage_packer", executable = True, cfg = "exec"),
        "_linux": attr.label(default = "@platforms//os:linux"),
        "_runtime_aarch64": attr.label(default = "@dx_appimage_tools//:runtime_aarch64", allow_single_file = True),
        "_runtime_x86_64": attr.label(default = "@dx_appimage_tools//:runtime_x86_64", allow_single_file = True),
        "_x86_64": attr.label(default = "@platforms//cpu:x86_64"),
    },
)
