"""mix format on the pinned Elixir and OTP runtimes."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "target_is_windows", "write_native_launcher")
load(":launcher.bzl", "launcher_preamble", "launcher_root", "windows_descriptor")

_EXEC = """PATH="$otp/bin:$elixir/bin:$PATH" exec "$elixir/bin/elixir" "$elixir/bin/mix" %s"$@"
"""

def mix_format_script(otp, elixir, mix_args = ["format"], inherit_env = False):
    """Returns the mix launcher for runfiles-relative OTP and Elixir roots; `mix_args` come before "$@"."""
    return launcher_preamble(otp, elixir, inherit_env) + _EXEC % "".join([arg + " " for arg in mix_args])

def _mix_format_impl(ctx):
    otp = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo
    elixir = ctx.toolchains["@rules_elixir//:toolchain_type"].elixirinfo
    otp_root = launcher_root(ctx, otp.version_file)
    elixir_root = launcher_root(ctx, elixir.version_file)
    if target_is_windows(ctx):
        descriptor = windows_descriptor(otp_root, elixir_root, [], elixir_root + "/bin/mix", ctx.attr.mix_args, ctx.attr.inherit_env)
        launcher, _, launcher_runfiles = write_native_launcher(ctx, descriptor)
    else:
        launcher = ctx.actions.declare_file(ctx.label.name)
        ctx.actions.write(launcher, mix_format_script(otp_root, elixir_root, ctx.attr.mix_args, ctx.attr.inherit_env), is_executable = True)
        launcher_runfiles = ctx.runfiles([launcher])
    runfiles = launcher_runfiles.merge(ctx.runfiles(transitive_files = depset(transitive = [otp.tree, elixir.tree])))
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

mix_format = rule(
    implementation = _mix_format_impl,
    doc = "Runs mix (format unless mix_args says otherwise) with no user home on the resolved Elixir and OTP.",
    executable = True,
    attrs = dict(NATIVE_LAUNCHER_ATTRS, mix_args = attr.string_list(default = ["format"]), inherit_env = attr.bool(default = False)),
    toolchains = [
        "@rules_elixir//:toolchain_type",
        "@rules_erlang//tools:toolchain_type",
    ],
)
