"""Explicit simulator boot, install, and launch launchers that never run at build time."""

load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("//apple/bundle:defs.bzl", "bundle_id_errors")

def simulator_errors(simulator):
    """Returns one error string when no simulator is declared."""
    if simulator == "":
        return ["apple_simulator: simulator names the target simulator by name or UDID"]
    return []

def _simulator_selection_impl(ctx):
    failures = simulator_errors(ctx.attr.simulator)
    if ctx.attr.check_bundle_id:
        failures.extend(bundle_id_errors("apple_simulator", ctx.attr.bundle_id))
    if len(failures) > 0:
        fail("; ".join(failures))
    return []

_simulator_selection = rule(
    implementation = _simulator_selection_impl,
    doc = "Fails analysis when a simulator launcher declaration is invalid.",
    attrs = {
        "bundle_id": attr.string(default = ""),
        "check_bundle_id": attr.bool(default = False),
        "simulator": attr.string(mandatory = True),
    },
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
    attrs = {"app": attr.label(mandatory = True)},
)

def _launcher(name, op, selection, env, data, visibility):
    sh_binary(
        name = name,
        srcs = [Label("//apple/ios:simctl_ops.sh")],
        data = [selection] + data,
        env = dict(env, DX_SIMCTL_OP = op),
        tags = ["manual"],
        visibility = visibility,
    )

def apple_simulator_boot(name, simulator = "", visibility = None):
    """Declares an explicit launcher that boots one simulator and waits for it."""
    _simulator_selection(
        name = name + "_selection",
        simulator = simulator,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _launcher(name, "boot", ":" + name + "_selection", {"DX_SIMCTL_DEVICE": simulator}, [], visibility)

def apple_simulator_install(name, app, simulator = "", visibility = None):
    """Declares an explicit launcher that installs one built app on one simulator."""
    _simulator_selection(
        name = name + "_selection",
        simulator = simulator,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _app_archive(
        name = name + "_app",
        app = app,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _launcher(
        name,
        "install",
        ":" + name + "_selection",
        {
            "DX_SIMCTL_APP": "$(rootpath :" + name + "_app)",
            "DX_SIMCTL_DEVICE": simulator,
        },
        [":" + name + "_app"],
        visibility,
    )

def apple_simulator_launch(name, bundle_id, simulator = "", visibility = None):
    """Declares an explicit launcher that starts one installed app on one simulator."""
    _simulator_selection(
        name = name + "_selection",
        bundle_id = bundle_id,
        check_bundle_id = True,
        simulator = simulator,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    _launcher(
        name,
        "launch",
        ":" + name + "_selection",
        {
            "DX_SIMCTL_BUNDLE_ID": bundle_id,
            "DX_SIMCTL_DEVICE": simulator,
        },
        [],
        visibility,
    )
