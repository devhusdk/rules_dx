"""Lets a prebuilt executable serve as the ppx of a rules_ocaml module."""

load("@rules_ocaml//build:providers.bzl", "OcamlExecutableMarker")

def _marker_impl(ctx):
    exe = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = exe, target_file = ctx.executable.exe, is_executable = True)
    return [
        DefaultInfo(
            executable = exe,
            files = depset([exe]),
            runfiles = ctx.runfiles([exe]).merge(ctx.attr.exe[DefaultInfo].default_runfiles),
        ),
        OcamlExecutableMarker(),
    ]

ocaml_executable_marker = rule(
    implementation = _marker_impl,
    doc = "Re-exports an executable with the OcamlExecutableMarker provider so rules_ocaml accepts it as a ppx.",
    attrs = {
        "exe": attr.label(executable = True, cfg = "exec", mandatory = True),
    },
    executable = True,
    provides = [DefaultInfo, OcamlExecutableMarker],
)
