"""The public per-language platform setting that consumer packages use as a select key."""

load("@bazel_skylib//lib:selects.bzl", "selects")
load("//core:platform_support.bzl", "lang_platforms")
load("//core/modules:langs.bzl", "LANGS")

visibility("private")

def lang_platform_setting(name):
    """Declares a public setting that matches any platform the named language's lang.json lists."""
    platforms = lang_platforms(name)
    for id, (os, cpu) in platforms.items():
        native.config_setting(
            name = name + "_" + id,
            constraint_values = [
                "@platforms//os:" + os,
                "@platforms//cpu:" + cpu,
            ],
        )

    selects.config_setting_group(
        name = name,
        match_any = [":" + name + "_" + id for id in platforms],
        visibility = ["//visibility:public"],
    )

def lang_unsupported(name):
    """Declares the constraint_value <lang>_unsupported that no platform sets, marking that language as unsupported."""
    lang = name.removesuffix("_unsupported")
    if lang == name or lang not in LANGS:
        fail("expected <lang>_unsupported for a language in LANGS, got " + name)
    native.constraint_setting(name = name + "_setting")

    native.constraint_value(
        name = name,
        constraint_setting = ":" + name + "_setting",
        visibility = ["//visibility:public"],
    )
