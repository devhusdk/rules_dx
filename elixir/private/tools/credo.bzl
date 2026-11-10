"""Credo on the pinned Elixir and OTP runtimes."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//core:rlocation.bzl", "runfiles_path")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "target_is_windows", "write_native_launcher")
load(":launcher.bzl", "elixir_descriptor", "launcher_key")

def _credo_impl(ctx):
    otp = ctx.toolchains["@rules_erlang//tools:toolchain_type"].otpinfo
    elixir = ctx.toolchains["@rules_elixir//:toolchain_type"].elixirinfo
    apps = {}
    for app in ctx.attr.apps:
        apps[app[ErlangAppInfo].app_name] = app[ErlangAppInfo]
        for dep in app[ErlangAppInfo].deps:
            apps[dep[ErlangAppInfo].app_name] = dep[ErlangAppInfo]

    # The toolchain's own Elixir stays authoritative; only tool ebins join the path.
    apps.pop("elixir", None)
    ebins = []
    files = []
    for name in sorted(apps):
        for beam in apps[name].beam:
            ebins.append(runfiles_path(ctx.workspace_name, beam.short_path))
            files.append(beam)
    driver = ctx.file.driver
    otp_key = launcher_key(ctx, otp.version_file)
    elixir_root = launcher_key(ctx, elixir.version_file).rpartition("/")[0]
    driver_key = runfiles_path(ctx.workspace_name, driver.short_path)
    launcher, _, launcher_runfiles = write_native_launcher(ctx, elixir_descriptor(otp_key, elixir_root, ebins, driver_key, target_is_windows(ctx)))
    runfiles = launcher_runfiles.merge(ctx.runfiles(
        files = [driver] + files,
        transitive_files = depset(transitive = [otp.tree, elixir.tree]),
    ))
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

credo = rule(
    implementation = _credo_impl,
    doc = "Runs the pinned Credo through its driver with no user home.",
    attrs = dict({
        "apps": attr.label_list(providers = [ErlangAppInfo], doc = "Credo and its dependencies."),
        "driver": attr.label(allow_single_file = [".exs"], mandatory = True, doc = "Driver script."),
    }, **NATIVE_LAUNCHER_ATTRS),
    executable = True,
    toolchains = [
        "@rules_elixir//:toolchain_type",
        "@rules_erlang//tools:toolchain_type",
    ],
)
