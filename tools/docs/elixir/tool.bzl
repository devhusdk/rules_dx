"""The elixir_dump.exs driver on the pinned Elixir and OTP runtimes."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "target_is_windows", "write_native_launcher")
load("//core:rlocation.bzl", "runfiles_path")
load("//elixir/private/tools:launcher.bzl", "elixir_descriptor", "launcher_key")

def _elixir_dump_tool_impl(ctx):
    otp = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo
    elixir = ctx.toolchains["@rules_elixir//:toolchain_type"].elixirinfo
    driver = ctx.file.driver
    elixir_root = launcher_key(ctx, elixir.version_file).rpartition("/")[0]
    driver_key = runfiles_path(ctx.workspace_name, driver.short_path)
    descriptor = elixir_descriptor(launcher_key(ctx, otp.version_file), elixir_root, [], driver_key, target_is_windows(ctx))
    launcher, _, launcher_runfiles = write_native_launcher(ctx, descriptor)
    runfiles = launcher_runfiles.merge(ctx.runfiles(
        files = [driver],
        transitive_files = depset(transitive = [otp.tree, elixir.tree]),
    ))
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

elixir_dump_tool = rule(
    implementation = _elixir_dump_tool_impl,
    doc = "Runs the docs dump driver on the pinned Elixir.",
    attrs = dict({
        "driver": attr.label(allow_single_file = [".exs"], mandatory = True, doc = "The elixir_dump.exs file."),
    }, **NATIVE_LAUNCHER_ATTRS),
    executable = True,
    toolchains = [
        "@rules_elixir//:toolchain_type",
        "@rules_erlang//tools:toolchain_type",
    ],
)
