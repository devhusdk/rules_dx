"""mix format on the pinned Elixir and OTP runtimes."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "target_is_windows", "write_native_launcher")
load(":launcher.bzl", "elixir_descriptor", "launcher_key")

def _mix_format_impl(ctx):
    otp = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo
    elixir = ctx.toolchains["@rules_elixir//:toolchain_type"].elixirinfo
    otp_key = launcher_key(ctx, otp.version_file)
    elixir_root = launcher_key(ctx, elixir.version_file).rpartition("/")[0]
    descriptor = elixir_descriptor(otp_key, elixir_root, [], elixir_root + "/bin/mix", target_is_windows(ctx), ctx.attr.mix_args, ctx.attr.inherit_env)
    launcher, _, launcher_runfiles = write_native_launcher(ctx, descriptor)
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
