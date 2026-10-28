"""The luals_tool rule: the pinned LuaLS release tree as one executable."""

_LAUNCHER = """#!/usr/bin/env bash
set -euo pipefail
runfiles="${{RUNFILES_DIR:-}}"
if [[ -z "$runfiles" ]]; then
  if [[ -d "$0.runfiles" ]]; then runfiles="$0.runfiles"; else runfiles="$(cd "$(dirname "$0")" && pwd -P)/$(basename "$0").runfiles"; fi
fi
# LuaLS finds main.lua and script/ next to the real binary, so resolve the runfiles symlink.
bin="$runfiles/{binary}"
if command -v realpath >/dev/null 2>&1; then bin="$(realpath "$bin")"; fi
exec "$bin" "$@"
"""

def _runfiles_path(workspace_name, file):
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return workspace_name + "/" + file.short_path

def _luals_tool_impl(ctx):
    binary = ctx.file.binary
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, _LAUNCHER.format(binary = _runfiles_path(ctx.workspace_name, binary)), is_executable = True)
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = ctx.runfiles(files = [launcher], transitive_files = depset(ctx.files.tree)),
    )]

luals_tool = rule(
    implementation = _luals_tool_impl,
    doc = "Runs the pinned LuaLS on its own release tree; `dx check` sets it with --@rules_dx//config:tool_luals.",
    executable = True,
    attrs = {
        "binary": attr.label(allow_single_file = True, mandatory = True, doc = "The LuaLS executable inside the release tree."),
        "tree": attr.label(mandatory = True, allow_files = True, doc = "Every file of the release tree: main.lua, script/, meta/ and locale/ must sit next to the binary."),
    },
)
