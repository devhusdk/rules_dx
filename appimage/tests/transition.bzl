"""Builds targets for the linux_arm64 cell from any host."""

load("//tools/testing:action_input.bzl", "platform_flip_transition")

_arm64 = platform_flip_transition(str(Label("//appimage:linux_arm64")))

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
