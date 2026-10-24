"""Rust crates with CXX-Qt bridges for Qt Quick applications."""

load("//rust/rules:defs.bzl", "rust_static_library")
load(":defs.bzl", "QT_REPOSITORY", "qt_rust_cc_library")

_CXX_CRATE = Label("//qt/rust:cxx")

_CXX_QT_CRATE = Label("//qt/rust:cxx_qt")

def qt_rust_library(
        name,
        bridges,
        srcs = [],
        crate_root = None,
        crate_name = None,
        crate_features = [],
        rustc_flags = [],
        deps = [],
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
    crate = {}
    for key in ["aspect_hints", "tags", "testonly", "target_compatible_with"]:
        if kwargs.get(key, None) != None:
            crate[key] = kwargs[key]
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
