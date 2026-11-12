"""The emulator serial a test machine declares with `--@rules_dx//android:emulator_serial`."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")

def _declared_emulator_impl(ctx):
    serial = ctx.attr._serial[BuildSettingInfo].value
    out = ctx.actions.declare_file(ctx.label.name + ".serial")
    ctx.actions.write(out, serial)
    return [DefaultInfo(files = depset([out]), runfiles = ctx.runfiles(files = [out]))]

declared_emulator = rule(
    implementation = _declared_emulator_impl,
    doc = "Writes the declared emulator serial to a file.",
    attrs = {
        "_serial": attr.label(default = "//android:emulator_serial", doc = "The emulator serial flag."),
    },
)
