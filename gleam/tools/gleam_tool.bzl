"""The pinned gleam command, with the pinned OTP and Node.js on PATH when asked."""

load("//gleam/rules:javascript.bzl", "GLEAM_TOOLCHAIN", "NODE_TOOLCHAIN", "runfiles_path")

ERLANG_TOOLCHAIN = "@rules_gleam//gleam_tools:erlang_toolchain_type"

_LAUNCHER = """#!/usr/bin/env bash
set -euo pipefail
if [[ -d "$0.runfiles" ]]; then
    runfiles="$0.runfiles"
elif [[ -n "${{RUNFILES_DIR:-}}" ]]; then
    runfiles="$RUNFILES_DIR"
elif [[ "$0" == *.runfiles/* ]]; then
    runfiles="${{0%%.runfiles/*}}.runfiles"
else
    echo "$0: no runfiles tree" >&2
    exit 2
fi
{path}exec "$runfiles/{gleam}" {command}"$@"
"""

def gleam_tool_script(gleam, command, bin_dirs):
    """Returns the launcher that runs gleam with a leading command and runtime bin dirs on PATH."""
    path = ""
    if bin_dirs:
        dirs = ":".join(['$(cd "$runfiles/{}" && pwd)'.format(d) for d in bin_dirs])
        path = 'export PATH="{}:$PATH"\n'.format(dirs)
    prefix = "".join(["'{}' ".format(arg) for arg in command])
    return _LAUNCHER.format(path = path, gleam = gleam, command = prefix)

def _gleam_tool_impl(ctx):
    gleam = ctx.toolchains[GLEAM_TOOLCHAIN].gleamtools.compiler
    files = [gleam]
    bin_dirs = []
    if ctx.attr.runtimes:
        erlang = ctx.toolchains[ERLANG_TOOLCHAIN].gleamerlangtools
        node = ctx.toolchains[NODE_TOOLCHAIN].nodeinfo.node
        files.extend(erlang.otp + [node])
        bin_dirs = [runfiles_path(ctx, erlang.erl).rpartition("/")[0], runfiles_path(ctx, node).rpartition("/")[0]]
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, gleam_tool_script(runfiles_path(ctx, gleam), ctx.attr.command, bin_dirs), is_executable = True)
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = ctx.runfiles([launcher] + files))]

gleam_tool = rule(
    implementation = _gleam_tool_impl,
    doc = "Runs the resolved gleam binary with a fixed leading command.",
    attrs = {
        "command": attr.string_list(doc = "Arguments placed before the caller's arguments."),
        "runtimes": attr.bool(doc = "Put the resolved OTP and Node.js on PATH."),
    },
    executable = True,
    toolchains = [ERLANG_TOOLCHAIN, GLEAM_TOOLCHAIN, NODE_TOOLCHAIN],
)
