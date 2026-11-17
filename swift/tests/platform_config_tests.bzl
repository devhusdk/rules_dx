"""Analysis test that platforms changes the configuration of the Swift upstream."""

load("//swift/private:defs.bzl", "SWIFT_PLATFORM_ATTRS", "swift_platforms_transition")
load("//tools/testing:platform_config.bzl", "platform_config_subject_rule", "platform_config_test")

_swift_platforms_subject = platform_config_subject_rule(swift_platforms_transition, SWIFT_PLATFORM_ATTRS)

def swift_platform_config_test(name, size):
    platform_config_test(
        name = name,
        size = size,
        subject_rule = _swift_platforms_subject,
        constraints = ["@platforms//os:linux", "@platforms//cpu:aarch64", "@platforms//cpu:x86_64"],
        subjects = {
            "aarch64": ({"platforms": [str(Label("//swift/private/platforms:exec_linux_aarch64"))]}, "linux,aarch64"),
            "x86_64": ({"platforms": [str(Label("//swift/private/platforms:exec_linux_x86_64"))]}, "linux,x86_64"),
        },
    )
