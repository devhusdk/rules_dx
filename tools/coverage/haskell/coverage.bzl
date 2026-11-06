"""LCOV coverage for Haskell test forwarders, converted from hpc."""

load("//haskell/toolchain:transition.bzl", "ghc_mingw_transition")
load("//libs/starlark:rlocation.bzl", "rlocation_path")

_SCRIPT = """#!/usr/bin/env bash
set -uo pipefail

rlocation() {{
  if [ -e "$TEST_SRCDIR/$1" ]; then
    echo "$TEST_SRCDIR/$1"
  else
    grep -m1 "^$1 " "$RUNFILES_MANIFEST_FILE" | cut -d ' ' -f 2-
  fi
}}

status=0
"$(rlocation {upstream})" "$@" || status=$?
if [ "$status" -ne 0 ] || [ -z "${{COVERAGE_DIR:-}}" ]; then
  exit "$status"
fi

tix=$(ls "$TEST_UNDECLARED_OUTPUTS_DIR"/*.tix 2>/dev/null | head -n 1)
if [ -z "$tix" ]; then
  echo "dx: the Haskell test wrote no .tix file" >&2
  exit 1
fi
mixdirs=()
for dir in "$TEST_UNDECLARED_OUTPUTS_DIR"/mix/*; do
  mixdirs+=("$dir")
done
"$(rlocation {lcov})" "$COVERAGE_DIR/haskell.dat" "$tix" "${{mixdirs[@]}}"
"""

def dx_haskell_coverage_attrs():
    """Returns the attributes Haskell coverage needs."""
    return {
        "_dx_hpc_lcov": attr.label(
            default = Label("//tools/coverage/haskell:hpc_lcov"),
            executable = True,
            cfg = ghc_mingw_transition,
        ),
    }


def dx_haskell_test_coverage(ctx, upstream):
    """Returns coverage providers that convert the upstream test's hpc data to LCOV."""
    upstream_exe = upstream[DefaultInfo].files_to_run.executable
    lcov = ctx.attr._dx_hpc_lcov[0][DefaultInfo]
    script = ctx.actions.declare_file(ctx.label.name + "_upstream_coverage.sh")
    ctx.actions.write(script, _SCRIPT.format(
        upstream = rlocation_path(ctx, upstream_exe),
        lcov = rlocation_path(ctx, lcov.files_to_run.executable),
    ), is_executable = True)
    instrumented = coverage_common.instrumented_files_info(
        ctx,
        source_attributes = ["srcs"],
        dependency_attributes = ["upstream"],
    )
    env = {}
    inherited = []
    if RunEnvironmentInfo in upstream:
        env.update(upstream[RunEnvironmentInfo].environment)
        inherited = upstream[RunEnvironmentInfo].inherited_environment
    return struct(
        instrumented = instrumented,
        env = RunEnvironmentInfo(environment = env, inherited_environment = inherited),
        runfiles = ctx.runfiles(files = [upstream_exe]).merge(lcov.default_runfiles),
        executable = script,
    )
