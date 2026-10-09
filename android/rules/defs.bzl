"""Bounded Android native library composition over qualified toolchains."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("//android/platforms:defs.bzl", "triple_for", "tuple_errors", "tuples")
load("//cc/rules:defs.bzl", "cc_binary", "cc_library")
load("//rust/rules:defs.bzl", "rust_static_library")

AndroidNativeInfo = provider(
    doc = "One Android native shared library with its target facts.",
    fields = ["api_level", "shared_library", "triple", "tuple"],
)

_ANDROID_PLATFORMS = {
    "device": "//android/platforms:android_device",
    "emulator": "//android/platforms:android_emulator",
}

def _android_transition_impl(_settings, attr):
    if attr.tuple not in _ANDROID_PLATFORMS:
        fail("android_native_library: unknown tuple '" + attr.tuple + "': want " + ", ".join(tuples()))
    return {"//command_line_option:platforms": [_ANDROID_PLATFORMS[attr.tuple]]}

android_transition = transition(
    implementation = _android_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _android_library_proxy_impl(ctx):
    failures = tuple_errors(ctx.attr.tuple, ctx.attr.api_level)
    if len(failures) > 0:
        fail("; ".join(failures))
    link = ctx.attr.link[0] if type(ctx.attr.link) == "list" else ctx.attr.link
    libraries = [file for file in link[DefaultInfo].files.to_list() if file.extension == "so"]
    if len(libraries) != 1:
        fail("android_native_library: '" + str(link.label) + "' built " + str(len(libraries)) +
             " .so files, want exactly one")
    merged = cc_common.merge_cc_infos(
        direct_cc_infos = [link[CcInfo]] if CcInfo in link else [],
    )
    triple = triple_for(ctx.attr.tuple)
    out = ctx.actions.declare_file(ctx.attr.name + ".so")
    ctx.actions.symlink(output = out, target_file = libraries[0])
    return [
        DefaultInfo(files = depset([out])),
        merged,
        AndroidNativeInfo(
            api_level = ctx.attr.api_level,
            shared_library = libraries[0],
            triple = triple,
            tuple = ctx.attr.tuple,
        ),
    ]

_android_library_proxy = rule(
    implementation = _android_library_proxy_impl,
    attrs = {
        "api_level": attr.int(default = 31),
        "link": attr.label(mandatory = True, cfg = android_transition),
        "tuple": attr.string(mandatory = True),
    },
)

def android_native_library(
        name,
        rust_srcs,
        c_srcs,
        hdrs = [],
        crate_name = None,
        edition = "2021",
        api_level = 31,
        crate_features = [],
        rustc_flags = [],
        copts = [],
        linkopts = [],
        visibility = None,
        **kwargs):
    """Builds one mixed Rust and C shared library per Android tuple."""
    rust_kwargs = dict(kwargs)
    if len(rust_srcs) == 1:
        rust_kwargs["crate_root"] = rust_srcs[0]
    rust_static_library(
        name = name + "_rust",
        srcs = rust_srcs,
        crate_name = crate_name,
        edition = edition,
        crate_features = crate_features,
        rustc_flags = rustc_flags,
        visibility = ["//visibility:private"],
        **rust_kwargs
    )
    cc_library(
        name = name + "_cc",
        srcs = c_srcs,
        hdrs = hdrs,
        copts = copts,
        visibility = ["//visibility:private"],
    )
    cc_binary(
        name = name + "_link",
        srcs = [],
        deps = [":" + name + "_rust", ":" + name + "_cc"],
        linkshared = True,
        linkopts = linkopts,
        visibility = ["//visibility:private"],
    )
    for tuple in tuples():
        _android_library_proxy(
            name = name + "_" + tuple,
            link = ":" + name + "_link",
            tuple = tuple,
            api_level = api_level,
            visibility = visibility,
            tags = ["manual"],
        )
