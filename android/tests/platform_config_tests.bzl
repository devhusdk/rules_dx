"""Analysis test that platforms changes the configuration of the Android native library."""

load("//android/private/library:defs.bzl", "android_transition")
load("//tools/testing:platform_config.bzl", "platform_config_subject_rule", "platform_config_test")

_android_subject = platform_config_subject_rule(android_transition, {
    "api_level": attr.int(doc = "Android API level.", default = 31),
    "platforms": attr.string_list(doc = "The one Android platform label."),
})

def android_platform_config_test(name, size):
    platform_config_test(
        name = name,
        size = size,
        subject_rule = _android_subject,
        constraints = ["@platforms//os:android", "@platforms//cpu:aarch64", "@platforms//cpu:x86_64"],
        subjects = {
            "device": ({"platforms": [str(Label("//android:android_device"))]}, "android,aarch64"),
            "emulator": ({"platforms": [str(Label("//android:android_emulator"))]}, "android,x86_64"),
        },
    )
