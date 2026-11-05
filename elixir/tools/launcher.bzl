"""Shared launcher pieces for Elixir tools that run on the pinned OTP and Elixir."""

load("//erlang/rules:defs.bzl", "runfiles_path")

MIX_HOME = "/nonexistent/dx_mix_format"

# A drive path that does not exist and is valid on Windows.
WINDOWS_MIX_HOME = "C:/dx_nonexistent/dx_mix_format"

# The runner clears PATH and erl.exe refuses to start without one.
WINDOWS_PATH = "C:/Windows/System32;C:/Windows"

def _mix_env(home):
    return {
        "ELIXIR_ERL_OPTIONS": "-elixir ansi_enabled false",
        "HEX_HOME": home + "/hex",
        "HEX_OFFLINE": "1",
        "HOME": home,
        "MIX_ARCHIVES": home + "/archives",
        "MIX_ENV": "dev",
        "MIX_HOME": home,
    }

MIX_FORMAT_ENV = _mix_env(MIX_HOME)

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

def launcher_preamble(otp, elixir, inherit_env = False):
    """Returns the launcher text up to the exec line for runfiles-relative OTP and Elixir roots.

    With `inherit_env` the mix and hex variables come from the caller instead of the no-home defaults.
    """
    env = "" if inherit_env else "\n".join(["export {}='{}'".format(key, value) for key, value in sorted(MIX_FORMAT_ENV.items())])
    return _PREAMBLE.format(otp = otp, elixir = elixir, env = env)

def launcher_root(ctx, version_file):
    """Returns the runfiles-relative directory that holds one toolchain version file."""
    return runfiles_path(ctx.workspace_name, version_file.short_path).rpartition("/")[0]

def windows_descriptor(otp, elixir, ebins, script, script_args = [], inherit_env = False):
    """Returns the native launcher descriptor that runs `script` on the pinned Elixir with erl.exe.

    `ebins` are extra runfiles-relative code paths; `script_args` follow the script.
    With `inherit_env` the mix and hex variables come from the caller.
    """
    lines = [
        "program=" + otp + "/bin/erl.exe",
        "arg=-noshell",
        "arg=-elixir_root",
        "path=" + elixir + "/lib",
        "arg=-pa",
        "path=" + elixir + "/lib/elixir/ebin",
    ]
    for ebin in ebins:
        lines += ["arg=-pa", "path=" + ebin]
    lines += ["arg=+pc", "arg=unicode", "arg=-s", "arg=elixir", "arg=start_cli", "arg=-extra", "path=" + script]
    lines += ["arg=" + arg for arg in script_args]
    env = {"PATH": WINDOWS_PATH} if inherit_env else _mix_env(WINDOWS_MIX_HOME) | {"PATH": WINDOWS_PATH}
    lines += ["env={}\t{}".format(key, value) for key, value in sorted(env.items())]
    return "\n".join(lines) + "\n"
