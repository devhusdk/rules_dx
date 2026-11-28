"""A Java toolchain proguard allowlister that refuses library proguard specs."""

_MESSAGE = "rules_dx: proguard_specs on java_library, java_import and java_plugin are unsupported; set them on android_library or android_binary"

def _unsupported_proguard_allowlister_impl(ctx):
    windows = ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])
    out = ctx.actions.declare_file(ctx.label.name + (".bat" if windows else ".sh"))
    ctx.actions.write(
        output = out,
        content = "@echo " + _MESSAGE + " 1>&2\r\n@exit /b 1\r\n" if windows else "#!/bin/sh\necho '" + _MESSAGE + "' >&2\nexit 1\n",
        is_executable = True,
    )
    return [DefaultInfo(executable = out)]

unsupported_proguard_allowlister = rule(
    implementation = _unsupported_proguard_allowlister_impl,
    attrs = {
        "_windows_os": attr.label(default = "@platforms//os:windows"),
    },
    executable = True,
    doc = "An allowlister that fails, so the Java toolchain needs no Python.",
)
