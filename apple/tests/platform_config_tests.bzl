"""Analysis test that the iOS cell environment and arch change the configuration of the cell."""

load("//apple/private/ios:defs.bzl", "ios_cell_transition")
load("//tools/testing:platform_config.bzl", "platform_config_subject_rule", "platform_config_test")

_ios_cell_subject = platform_config_subject_rule(ios_cell_transition, {
    "arch": attr.string(doc = "Target architecture."),
    "deployment": attr.string(doc = "Minimum iOS deployment version."),
    "environment": attr.string(doc = "device or simulator."),
    "xcode_version": attr.string(doc = "Xcode version."),
})

def apple_platform_config_test(name, size):
    platform_config_test(
        name = name,
        size = size,
        subject_rule = _ios_cell_subject,
        constraints = ["@platforms//os:ios", "@platforms//cpu:aarch64", "@platforms//cpu:x86_64"],
        subjects = {
            "device": ({"arch": "arm64", "environment": "device"}, "ios,aarch64"),
            "simulator": ({"arch": "x86_64", "environment": "simulator"}, "ios,x86_64"),
        },
    )
