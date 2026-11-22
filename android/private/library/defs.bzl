"""Bounded Android native library composition over qualified toolchains."""

load("@rules_android//rules:rules.bzl", _android_library = "android_library")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_kotlin//kotlin:android.bzl", _kt_android_library = "kt_android_library")
load("//android/private/platforms:defs.bzl", "effective_api_level", "platform_label")
load("//android/private/platforms:validate.bzl", "android_tuple_errors", "android_tuples")
load("//cc:defs.bzl", "cc_binary", "cc_library")
load("//core:cells.bzl", "unsupported_constraint")
load("//core:platform.bzl", "platforms_transition")
load("//core/quality:sources.bzl", "single_target")
load("//rust:defs.bzl", "rust_static_library")
load("//rust/private:edition.bzl", "RUST_EDITION")
load(":kwargs.bzl", "android_split_native_kwargs")

_ANDROID_PLATFORMS = {name: platform_label(name) for name in android_tuples()}

_API_LEVEL_SETTING = str(Label("//android:api_level"))

_TUPLES_BY_PLATFORM = {label: name for name, label in _ANDROID_PLATFORMS.items()}

def _android_settings(_settings, attr, _platforms):
    return {_API_LEVEL_SETTING: effective_api_level(attr.api_level)}

android_transition = platforms_transition(
    _android_settings,
    outputs = [_API_LEVEL_SETTING],
    single = "android_native_library",
)

def _android_library_proxy_impl(ctx):
    failures = android_tuple_errors(ctx.attr.tuple, ctx.attr.api_level)
    if len(failures) > 0:
        fail("; ".join(failures))
    link = single_target(ctx.attr.link)
    libraries = [file for file in link[DefaultInfo].files.to_list() if file.extension == "so"]
    if len(libraries) != 1:
        fail("android_native_library: '" + str(link.label) + "' built " + str(len(libraries)) +
             " .so files, want exactly one")
    merged = cc_common.merge_cc_infos(
        direct_cc_infos = [link[CcInfo]] if CcInfo in link else [],
    )
    out = ctx.actions.declare_file(ctx.attr.name + ".so")
    ctx.actions.symlink(output = out, target_file = libraries[0])
    return [
        DefaultInfo(files = depset([out])),
        merged,
    ]

_android_library_proxy = rule(
    implementation = _android_library_proxy_impl,
    doc = "Exposes one native library built for an Android tuple.",
    attrs = {
        "api_level": attr.int(
            doc = "Android API level the native library is compiled and linked against.",
            default = 31,
        ),
        "link": attr.label(
            doc = "Native library target built for the Android tuple.",
            mandatory = True,
            cfg = android_transition,
        ),
        "platforms": attr.string_list(
            doc = "The one Android platform label the native library is built for.",
            mandatory = True,
        ),
        "tuple": attr.string(
            doc = "Supported Android native tuple that selects the target platform.",
            mandatory = True,
        ),
    },
)

def android_native_library(
        name,
        rust_srcs,
        c_srcs,
        hdrs = [],
        crate_name = None,
        edition = RUST_EDITION,
        platforms = None,
        api_level = 31,
        crate_features = None,
        rustc_flags = [],
        copts = [],
        linkopts = [],
        visibility = None,
        **kwargs):
    """Builds one mixed Rust and C shared library per Android tuple."""
    crate_features = crate_features or []
    labels = [_ANDROID_PLATFORMS[t] for t in android_tuples()] if platforms == None else [str(native.package_relative_label(p)) for p in platforms]
    unknown = [label for label in labels if label not in _TUPLES_BY_PLATFORM]
    if len(labels) == 0 or len(unknown) > 0:
        fail("android_native_library: platforms must be a non-empty subset of " + ", ".join([_ANDROID_PLATFORMS[t] for t in android_tuples()]) +
             (", got unknown " + ", ".join(unknown) if len(unknown) > 0 else ""))
    tuples = [_TUPLES_BY_PLATFORM[label] for label in labels]
    shared, rust_kwargs = android_split_native_kwargs(kwargs)
    proxy_shared = _sdk_kwargs(shared)
    proxy_shared["tags"] = sorted({tag: True for tag in shared.get("tags", []) + ["manual"]}.keys())
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
        **dict(rust_kwargs, **shared)
    )
    cc_library(
        name = name + "_cc",
        srcs = c_srcs,
        hdrs = hdrs,
        copts = copts,
        visibility = ["//visibility:private"],
        **shared
    )
    cc_binary(
        name = name + "_link",
        srcs = [],
        deps = [":" + name + "_rust", ":" + name + "_cc"],
        linkshared = True,
        linkopts = linkopts,
        visibility = ["//visibility:private"],
        **shared
    )
    for tuple in tuples:
        _android_library_proxy(
            name = name + "_" + tuple,
            link = ":" + name + "_link",
            platforms = [_ANDROID_PLATFORMS[tuple]],
            tuple = tuple,
            api_level = api_level,
            visibility = visibility,
            **proxy_shared
        )
    native.filegroup(
        name = name,
        srcs = [name + "_" + tuple for tuple in tuples],
        visibility = visibility,
        **_sdk_kwargs(shared)
    )

SDK_COMPATIBLE_WITH = select({
    Label("//core/platforms:linux_arm64"): [Label("@platforms//:incompatible"), unsupported_constraint("android")],
    "//conditions:default": [],
})

def _sdk_kwargs(kwargs):
    return kwargs | {"target_compatible_with": kwargs.get("target_compatible_with", []) + SDK_COMPATIBLE_WITH}

def android_java_library(**kwargs):
    """Android Java library over rules_android android_library."""
    _android_library(**_sdk_kwargs(kwargs))

def android_kotlin_library(**kwargs):
    """Android Kotlin library over rules_kotlin kt_android_library."""
    _kt_android_library(**_sdk_kwargs(kwargs))
