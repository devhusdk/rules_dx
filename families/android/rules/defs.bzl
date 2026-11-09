"""Bounded Android native library composition over qualified toolchains."""

load("@rules_android//rules:rules.bzl", _android_library = "android_library")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_dx//cc/rules:defs.bzl", "cc_binary", "cc_library")
load("@rules_dx//rust/rules:defs.bzl", "rust_static_library")
load("@rules_dx//rust/rules:edition.bzl", "RUST_EDITION")
load("@rules_kotlin//kotlin:android.bzl", _kt_android_library = "kt_android_library")
load("//platforms:defs.bzl", "effective_api_level", "platform_label", "tuple_errors", "tuples")

_ANDROID_PLATFORMS = {name: platform_label(name) for name in tuples()}

_API_LEVEL_SETTING = str(Label("//platforms:api_level"))

_SHARED_KWARGS = [
    "compatible_with",
    "deprecation",
    "exec_compatible_with",
    "features",
    "restricted_to",
    "tags",
    "target_compatible_with",
    "testonly",
]

def split_native_kwargs(kwargs):
    """Splits kwargs into attributes for every generated target and the rest."""
    shared = {key: kwargs[key] for key in _SHARED_KWARGS if key in kwargs}
    rest = {key: value for key, value in kwargs.items() if key not in shared}
    return shared, rest

def _android_transition_impl(_settings, attr):
    if attr.tuple not in _ANDROID_PLATFORMS:
        fail("android_native_library: unknown tuple '" + attr.tuple + "': want " + ", ".join(tuples()))
    return {
        "//command_line_option:platforms": [_ANDROID_PLATFORMS[attr.tuple]],
        _API_LEVEL_SETTING: effective_api_level(attr.api_level),
    }

android_transition = transition(
    implementation = _android_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms", _API_LEVEL_SETTING],
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
        crate_features = [],
        rustc_flags = [],
        copts = [],
        linkopts = [],
        visibility = None,
        **kwargs):
    """Builds one mixed Rust and C shared library per Android tuple.

    `platforms` lists the tuples to build (default all). `name_<tuple>` is the
    library for one tuple and `name` groups them.
    """
    platforms = tuples() if platforms == None else platforms
    unknown = [tuple for tuple in platforms if tuple not in _ANDROID_PLATFORMS]
    if len(platforms) == 0 or len(unknown) > 0:
        fail("android_native_library: platforms must be a non-empty subset of " + ", ".join(tuples()) +
             (", got unknown " + ", ".join(unknown) if len(unknown) > 0 else ""))
    shared, rust_kwargs = split_native_kwargs(kwargs)
    proxy_shared = dict(shared)
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
    for tuple in platforms:
        _android_library_proxy(
            name = name + "_" + tuple,
            link = ":" + name + "_link",
            tuple = tuple,
            api_level = api_level,
            visibility = visibility,
            **proxy_shared
        )
    native.filegroup(
        name = name,
        srcs = [name + "_" + tuple for tuple in platforms],
        visibility = visibility,
        **shared
    )

def java_android_library(**kwargs):
    """Android Java library over rules_android android_library."""
    _android_library(**kwargs)

def kotlin_android_library(**kwargs):
    """Android Kotlin library over rules_kotlin kt_android_library."""
    _kt_android_library(**kwargs)
