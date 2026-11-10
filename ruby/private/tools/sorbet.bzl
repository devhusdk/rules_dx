"""The sorbet_tool rule: the pinned Sorbet binary as one executable."""

load("//core:rlocation.bzl", "rlocation_path")

_LAUNCHER = """#!/usr/bin/env bash
set -euo pipefail
runfiles="${{RUNFILES_DIR:-}}"
if [[ -z "$runfiles" ]]; then
  if [[ -d "$0.runfiles" ]]; then runfiles="$0.runfiles"; else runfiles="$(cd "$(dirname "$0")" && pwd -P)/$(basename "$0").runfiles"; fi
fi
exec "$runfiles/{binary}" "$@"
"""


def _sorbet_tool_impl(ctx):
    binary = ctx.file.binary
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, _LAUNCHER.format(binary = rlocation_path(ctx, binary)), is_executable = True)
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = ctx.runfiles(files = [launcher, binary]),
    )]

sorbet_tool = rule(
    implementation = _sorbet_tool_impl,
    doc = "Runs the pinned Sorbet; the default of --@rules_dx//config:tool_sorbet.",
    executable = True,
    attrs = {
        "binary": attr.label(allow_single_file = True, mandatory = True, doc = "The sorbet executable."),
    },
)
