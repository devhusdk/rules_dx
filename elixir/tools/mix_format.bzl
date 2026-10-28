"""mix format on the pinned Elixir and OTP runtimes."""

load(":launcher.bzl", "launcher_preamble", "launcher_root")

_EXEC = """PATH="$otp/bin:$elixir/bin:$PATH" exec "$elixir/bin/elixir" "$elixir/bin/mix" format "$@"
"""

def mix_format_script(otp, elixir):
    """Returns the mix format launcher for runfiles-relative OTP and Elixir roots."""
    return launcher_preamble(otp, elixir) + _EXEC

def _mix_format_impl(ctx):
    otp = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo
    elixir = ctx.toolchains["@rules_elixir//:toolchain_type"].elixirinfo
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, mix_format_script(launcher_root(ctx, otp.version_file), launcher_root(ctx, elixir.version_file)), is_executable = True)
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
