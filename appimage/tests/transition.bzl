"""Builds targets for the linux_arm64 cell from any host."""

def _arm64_impl(settings, attr):
    _ = settings
    _ = attr
    return {"//command_line_option:platforms": str(Label("//appimage/platforms:linux_arm64"))}

_arm64 = transition(
    implementation = _arm64_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _files_impl(ctx):
    files = []
    for target in ctx.attr.srcs:
        files.extend(target[DefaultInfo].files.to_list())
    return [DefaultInfo(files = depset(files))]

arm64_files = rule(
    implementation = _files_impl,
    doc = "Forwards the files of targets built for linux_arm64.",
    attrs = {
        "srcs": attr.label_list(doc = "Targets built for linux_arm64.", cfg = _arm64),
    },
)
