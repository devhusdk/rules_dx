"""Credo on the pinned Elixir and OTP runtimes."""

load("@rules_erlang//:erlang_app_info.bzl", "ErlangAppInfo")
load("//erlang/rules:defs.bzl", "runfiles_path")
load(":launcher.bzl", "launcher_preamble", "launcher_root")

_EXEC = """PATH="$otp/bin:$elixir/bin:$PATH" exec "$elixir/bin/elixir"{paths} "$runfiles/{driver}" "$@"
"""

def credo_script(otp, elixir, ebins, driver):
    """Returns the Credo launcher for runfiles-relative roots, tool ebin directories and driver."""
    paths = "".join([' -pa "$runfiles/{}"'.format(ebin) for ebin in ebins])
    return launcher_preamble(otp, elixir) + _EXEC.format(paths = paths, driver = driver)

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
    launcher = ctx.actions.declare_file(ctx.label.name)
    script = credo_script(
        launcher_root(ctx, otp.version_file),
        launcher_root(ctx, elixir.version_file),
        ebins,
        runfiles_path(ctx.workspace_name, driver.short_path),
    )
    ctx.actions.write(launcher, script, is_executable = True)
    runfiles = ctx.runfiles(
        files = [launcher, driver] + files,
        transitive_files = depset(transitive = [otp.tree, elixir.tree]),
    )
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

credo = rule(
    implementation = _credo_impl,
    doc = "Runs the pinned Credo through its driver with no user home.",
    attrs = {
        "apps": attr.label_list(providers = [ErlangAppInfo], doc = "Credo and its dependencies."),
        "driver": attr.label(allow_single_file = [".exs"], mandatory = True, doc = "Driver script."),
    },
    executable = True,
    toolchains = [
        "@rules_elixir//:toolchain_type",
        "@rules_erlang//tools:toolchain_type",
    ],
)
