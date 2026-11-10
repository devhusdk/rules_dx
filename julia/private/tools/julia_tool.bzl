"""Runs a Julia binary under the Julia version the formatter Manifest was resolved for."""

load("@rules_julia//julia:julia_info.bzl", "JuliaInfo")
load("//modules:versions.bzl", "JULIA_VERSION")

_VERSION_SETTING = "@rules_julia//julia/settings:version"

def _pin_version_impl(_settings, _attr):
    return {_VERSION_SETTING: JULIA_VERSION}

_pin_version = transition(
    implementation = _pin_version_impl,
    inputs = [],
    outputs = [_VERSION_SETTING],
)

def _dx_julia_tool_impl(ctx):
    binary = ctx.attr.binary[0]
    executable = ctx.executable.binary
    link = ctx.actions.declare_file(ctx.label.name + ("." + executable.extension if executable.extension else ""))
    ctx.actions.symlink(target_file = executable, output = link, is_executable = True)
    return [
        DefaultInfo(
            executable = link,
            files = depset([link]),
            runfiles = binary[DefaultInfo].default_runfiles,
        ),
        binary[JuliaInfo],
    ]

dx_julia_tool = rule(
    implementation = _dx_julia_tool_impl,
    attrs = {
        "binary": attr.label(
            cfg = _pin_version,
            providers = [JuliaInfo],
            executable = True,
            doc = "The julia_binary to run.",
            mandatory = True,
        ),
    },
    executable = True,
    doc = "A julia_binary pinned to the Julia version of the formatter Manifest.",
)
