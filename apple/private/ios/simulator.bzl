"""Explicit simulator boot, install, and launch launchers that never run at build time."""

load("//apple/private/bundle:validate.bzl", "bundle_id_errors")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")

def simulator_errors(simulator):
    """Returns one error string when no simulator is declared."""
    if simulator == "":
        return ["apple_simulator: simulator names the target simulator by name or UDID"]
    return []

def _simctl_impl(ctx):
    failures = simulator_errors(ctx.attr.simulator)
    if ctx.attr.check_bundle_id:
        failures.extend(bundle_id_errors("apple_simulator", ctx.attr.bundle_id))
    if len(failures) > 0:
        fail("; ".join(failures))
    xcode = ctx.attr._xcode_config[apple_common.XcodeVersionConfig].xcode_version()
    lines = [
        "program=" + rlocation_path(ctx, ctx.executable._ops),
        "arg=simctl",
        "arg=" + ctx.attr.op,
        "env=DX_XCODE_VERSION\t" + (str(xcode) if xcode else ""),
        "env=DX_SIMCTL_DEVICE\t" + ctx.attr.simulator,
    ]
    runfiles = [ctx.attr._ops[DefaultInfo].default_runfiles]
    if ctx.attr.bundle_id:
        lines.append("env=DX_SIMCTL_BUNDLE_ID\t" + ctx.attr.bundle_id)
    if ctx.attr.app:
        app = ctx.file.app
        lines.append("path=" + rlocation_path(ctx, app))
        runfiles.append(ctx.runfiles(files = [app]))
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "\n".join(lines) + "\n")
    return DefaultInfo(executable = launcher, runfiles = launcher_runfiles.merge_all(runfiles))

_simctl = rule(
    implementation = _simctl_impl,
    doc = "A launcher that runs one simctl operation on one simulator.",
    attrs = {
        "app": attr.label(
            doc = "The single built .ipa or .app to install.",
            allow_single_file = True,
        ),
        "bundle_id": attr.string(
            doc = "Bundle identifier of the app on the simulator, validated when requested.",
            default = "",
        ),
        "check_bundle_id": attr.bool(
            doc = "Whether to validate bundle_id as a reverse DNS identifier.",
            default = False,
        ),
        "op": attr.string(
            doc = "The simctl operation: boot, install or launch.",
            mandatory = True,
            values = ["boot", "install", "launch"],
        ),
        "simulator": attr.string(
            doc = "Name or UDID of the simulator to target.",
            mandatory = True,
        ),
        "_ops": attr.label(
            default = Label("//apple/private/tools/ops:apple_ops"),
            executable = True,
            cfg = "target",
        ),
        "_xcode_config": attr.label(
            default = configuration_field(
                name = "xcode_config_label",
                fragment = "apple",
            ),
        ),
    } | NATIVE_LAUNCHER_ATTRS,
    executable = True,
    fragments = ["apple"],
)

def _app_archive_impl(ctx):
    bundles = [
        file
        for file in ctx.attr.app[DefaultInfo].files.to_list()
        if file.extension in ["ipa", "app"]
    ]
    if len(bundles) != 1:
        fail("apple_simulator: '{}' built {} .ipa or .app outputs, want exactly one".format(ctx.attr.app.label, len(bundles)))
    return [DefaultInfo(files = depset(bundles))]

_app_archive = rule(
    implementation = _app_archive_impl,
    doc = "Exposes the one .ipa or .app an iOS app builds.",
    attrs = {"app": attr.label(
        doc = "App target whose single built ipa or app output is installed.",
        mandatory = True,
    )},
)

def apple_simulator_boot(name, simulator = "", visibility = None):
    """Declares an explicit launcher that boots one simulator and waits for it."""
    _simctl(
        name = name,
        op = "boot",
        simulator = simulator,
        tags = ["manual"],
        visibility = visibility,
    )

def apple_simulator_install(name, app, simulator = "", visibility = None):
    """Declares an explicit launcher that installs one built app on one simulator."""
    _app_archive(
        name = name + "_app",
        app = app,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _simctl(
        name = name,
        app = ":" + name + "_app",
        op = "install",
        simulator = simulator,
        tags = ["manual"],
        visibility = visibility,
    )

def apple_simulator_launch(name, bundle_id, simulator = "", visibility = None):
    """Declares an explicit launcher that starts one installed app on one simulator."""
    _simctl(
        name = name,
        bundle_id = bundle_id,
        check_bundle_id = True,
        op = "launch",
        simulator = simulator,
        tags = ["manual"],
        visibility = visibility,
    )
