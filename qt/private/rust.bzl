"""Rust crates with CXX-Qt bridges for Qt Quick applications."""

load("//rust:defs.bzl", "rust_static_library")
load(":defs.bzl", "QT_COMPATIBLE_WITH", "QT_REPOSITORY", "qt_rust_cc_library")

_CXX_CRATE = Label("//qt/private/rust:cxx")

_CXX_QT_CRATE = Label("//qt/private/rust:cxx_qt")

def qt_rust_library(
        name,
        bridges,
        srcs = None,
        crate_root = None,
        crate_name = None,
        crate_features = None,
        rustc_flags = [],
        deps = None,
        cc_deps = [],
        cxx = _CXX_CRATE,
        cxx_qt = _CXX_QT_CRATE,
        generator = None,
        uri = None,
        version = "1.0",
        qml_srcs = [],
        qml_depends = [],
        modules = None,
        qt = QT_REPOSITORY,
        copts = [],
        visibility = None,
        **kwargs):
    """Declares a Rust crate with CXX-Qt bridges and the Qt C++ library they generate."""
    srcs = srcs or []
    crate_features = crate_features or []
    deps = deps or []
    crate = {}
    for key in ["aspect_hints", "tags", "testonly"]:
        if kwargs.get(key, None) != None:
            crate[key] = kwargs[key]
    crate["target_compatible_with"] = kwargs.get("target_compatible_with", []) + QT_COMPATIBLE_WITH
    if crate_root != None:
        crate["crate_root"] = crate_root
    rust_static_library(
        name = name + "_rust",
        srcs = srcs + [src for src in bridges if src not in srcs],
        crate_name = crate_name or name,
        crate_features = crate_features,
        rustc_flags = rustc_flags,
        deps = deps + [cxx, cxx_qt],
        visibility = visibility,
        **crate
    )
    qt_rust_cc_library(
        name = name,
        bridges = bridges,
        rust = ":" + name + "_rust",
        cc_deps = cc_deps,
        generator = generator,
        uri = uri,
        version = version,
        qml_srcs = qml_srcs,
        qml_depends = qml_depends,
        modules = modules,
        qt = qt,
        copts = copts,
        visibility = visibility,
        **kwargs
    )
