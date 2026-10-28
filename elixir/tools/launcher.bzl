"""Shared launcher pieces for Elixir tools that run on the pinned OTP and Elixir."""

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

_PREAMBLE = """#!/usr/bin/env bash
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
"""

def launcher_preamble(otp, elixir):
    """Returns the launcher text up to the exec line for runfiles-relative OTP and Elixir roots."""
    env = "\n".join(["export {}='{}'".format(key, value) for key, value in sorted(MIX_FORMAT_ENV.items())])
    return _PREAMBLE.format(otp = otp, elixir = elixir, env = env)

def launcher_root(ctx, version_file):
    """Returns the runfiles-relative directory that holds one toolchain version file."""
    return runfiles_path(ctx.workspace_name, version_file.short_path).rpartition("/")[0]
