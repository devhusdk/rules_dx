"""Shared launcher pieces for Elixir tools that run on the pinned OTP and Elixir."""

load("//core:rlocation.bzl", "runfiles_path")

MIX_HOME = "/nonexistent/dx_mix_format"

# A drive path that does not exist and is valid on Windows.
WINDOWS_MIX_HOME = "C:/dx_nonexistent/dx_mix_format"

# What the caller of a `caller_homes` launcher may set; the rest of the mix environment is fixed.
_CALLER_HOMES = ["HEX_HOME", "HOME", "MIX_HOME"]

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

def launcher_key(ctx, version_file):
    """Returns the runfiles key of one toolchain version file."""
    return runfiles_path(ctx.workspace_name, version_file.short_path)

def elixir_descriptor(otp_key, elixir, ebins, script, windows, script_args = [], caller_homes = False):
    """Returns the native launcher descriptor that runs `script` on the pinned Elixir with erl."""
    otp = otp_key.rpartition("/")[0]
    program = otp + "/bin/erl" + (".exe" if windows else "")
    lines = [
        "program=" + program,
        "arg=-noshell",
        "arg=-elixir_root",
        "path=" + elixir + "/lib",
        "arg=-pa",
        "path=" + elixir + "/lib/elixir/ebin",
    ]
    for ebin in ebins:
        lines += ["arg=-pa", "path=" + ebin]
    lines += ["arg=+pc", "arg=unicode"]
    if not windows:
        lines += ["arg=+fnu", "arg=-kernel", "arg=standard_io_encoding", "arg=unicode"]
    lines += ["arg=-s", "arg=elixir", "arg=start_cli", "arg=-extra", "path=" + script]
    lines += ["arg=" + arg for arg in script_args]
    if windows:
        lines.append("env_dir=PATH\t" + program)
    else:
        lines.append("env_dir=ERL_ROOTDIR\t" + otp_key)
    env = _mix_env(WINDOWS_MIX_HOME if windows else MIX_HOME)
    if caller_homes:
        homes = {key: env.pop(key) for key in _CALLER_HOMES}
        env.pop("HEX_OFFLINE")
        env.pop("MIX_ARCHIVES")
        env.pop("MIX_ENV")
        lines += ["env_default={}\t{}".format(key, value) for key, value in sorted(homes.items())]
    lines += ["env={}\t{}".format(key, value) for key, value in sorted(env.items())]
    return "\n".join(lines) + "\n"
