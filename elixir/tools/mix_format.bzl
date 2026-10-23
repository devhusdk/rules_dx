"""mix format on the pinned Elixir and OTP runtimes."""

load("//erlang/rules:defs.bzl", "runfiles_path")

MIX_HOME = "/nonexistent/dx_mix_format"

MIX_FORMAT_ENV = {
    "ELIXIR_ERL_OPTIONS": "-elixir ansi_enabled false",
    "HEX_HOME": MIX_HOME + "/hex",
    "HEX_OFFLINE": "1",
    "HOME": MIX_HOME,
    "MIX_ARCHIVES": MIX_HOME + "/archives",
    "MIX_ENV": "dev",
    "MIX_HOME": MIX_HOME,
}

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
otp="$(cd "$runfiles/{otp}" && pwd)"
elixir="$(cd "$runfiles/{elixir}" && pwd)"
{env}
PATH="$otp/bin:$elixir/bin:$PATH" exec "$elixir/bin/elixir" "$elixir/bin/mix" format "$@"
"""

def mix_format_script(otp, elixir):
    """Returns the mix format launcher for runfiles-relative OTP and Elixir roots."""
    env = "\n".join(["export {}='{}'".format(key, value) for key, value in sorted(MIX_FORMAT_ENV.items())])
    return _LAUNCHER.format(otp = otp, elixir = elixir, env = env)

def _root(ctx, version_file):
    return runfiles_path(ctx.workspace_name, version_file.short_path).rpartition("/")[0]

def _mix_format_impl(ctx):
    otp = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo
    elixir = ctx.toolchains["@rules_elixir//:toolchain_type"].elixirinfo
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, mix_format_script(_root(ctx, otp.version_file), _root(ctx, elixir.version_file)), is_executable = True)
    runfiles = ctx.runfiles([launcher], transitive_files = depset(transitive = [otp.tree, elixir.tree]))
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

mix_format = rule(
    implementation = _mix_format_impl,
    doc = "Runs mix format with no user home on the resolved Elixir and OTP.",
    executable = True,
    toolchains = [
        "@rules_elixir//:toolchain_type",
        "@rules_erlang//tools:toolchain_type",
    ],
)
