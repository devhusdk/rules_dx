"""Builds a flutter_application in profile mode."""

def _profile_impl(_settings, _attr):
    return {
        "//command_line_option:compilation_mode": "opt",
        "@rules_flutter//flutter:profile": True,
    }

_profile = transition(
    implementation = _profile_impl,
    inputs = [],
    outputs = [
        "//command_line_option:compilation_mode",
        "@rules_flutter//flutter:profile",
    ],
)

def _profile_app_impl(ctx):
    files = ctx.attr.app[0][DefaultInfo].files
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

profile_app = rule(
    implementation = _profile_app_impl,
    doc = "Links the files of one flutter_application built in profile mode.",
    attrs = {"app": attr.label(mandatory = True, cfg = _profile)},
)
