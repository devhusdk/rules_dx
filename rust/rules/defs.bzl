"""Experimental minimal Rust wrappers."""

load("@crates//:crates.bzl", _aliases = "aliases", _crate_deps = "crate_deps")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", _rust_binary = "rust_binary", _rust_clippy_test = "rust_clippy_test", _rust_common = "rust_common", _rust_library = "rust_library", _rust_proc_macro = "rust_proc_macro", _rust_shared_library = "rust_shared_library", _rust_static_library = "rust_static_library", _rust_test = "rust_test", _rustfmt_test = "rustfmt_test")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "RUST")
load(":edition.bzl", "RUST_EDITION")

_DX_FORWARD_PROVIDES = [
    _rust_common.crate_info,
    _rust_common.dep_info,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_RUST_SOURCE_SPECS = [(RUST, "rs")]
_DX_RUST_SOURCE_EXTS = [".rs"]
_DX_RUST_CRATE_PROVIDERS = [(_rust_common.crate_info, "CrateInfo"), (_rust_common.dep_info, "DepInfo")]

_rust_forward = dx_library_forward_rule(
    provides = _DX_FORWARD_PROVIDES,
    required_providers = _DX_RUST_CRATE_PROVIDERS,
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [
        [_rust_common.crate_info],
        [_rust_common.test_crate_info],
    ],
)

_DX_CC_FORWARD_PROVIDES = [
    _rust_common.test_crate_info,
    _rust_common.dep_info,
    CcInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_rust_forward_cc = dx_library_forward_rule(
    provides = _DX_CC_FORWARD_PROVIDES,
    required_providers = [(_rust_common.test_crate_info, "TestCrateInfo"), (_rust_common.dep_info, "DepInfo"), (CcInfo, "CcInfo")],
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [[_rust_common.test_crate_info]],
)

_rust_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_FORWARD_PROVIDES,
    required_providers = _DX_RUST_CRATE_PROVIDERS,
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [
        [_rust_common.crate_info],
        [_rust_common.test_crate_info],
    ],
)

_rust_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_FORWARD_PROVIDES,
    required_providers = _DX_RUST_CRATE_PROVIDERS,
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [
        [_rust_common.crate_info],
        [_rust_common.test_crate_info],
    ],
    extra_attrs = dx_lcov_merger_attr(),
)

def rust_library(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Experimental minimal wrapper over rust_library."""
    dx_wrap(
        name,
        _rust_library,
        _rust_forward,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_binary(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Experimental minimal wrapper over rust_binary."""
    dx_wrap(
        name,
        _rust_binary,
        _rust_forward_binary,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_test(
        name,
        srcs = None,
        crate = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Experimental minimal wrapper over rust_test."""
    upstream_kwargs = dict(kwargs)
    upstream_kwargs["crate"] = crate
    upstream_kwargs["edition"] = edition
    upstream_kwargs.setdefault("crate_name", name)
    dx_wrap_test(name, _rust_test, _rust_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **kwargs)

def rust_proc_macro(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Experimental minimal wrapper over rust_proc_macro."""
    dx_wrap(
        name,
        _rust_proc_macro,
        _rust_forward,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_shared_library(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Experimental minimal wrapper over rust_shared_library."""
    dx_wrap(
        name,
        _rust_shared_library,
        _rust_forward_cc,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_static_library(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Experimental minimal wrapper over rust_static_library."""
    dx_wrap(
        name,
        _rust_static_library,
        _rust_forward_cc,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rustfmt_test(name, targets, size = "small", **kwargs):
    """Thin wrapper over upstream rustfmt_test."""
    _rustfmt_test(
        name = name,
        targets = targets,
        size = size,
        **kwargs
    )

def rust_clippy_test(name, targets, size = "small", **kwargs):
    """Thin wrapper over upstream rust_clippy_test."""
    _rust_clippy_test(
        name = name,
        targets = targets,
        size = size,
        **kwargs
    )

def dx_rust_crate(
        name,
        package_name,
        deps,
        dev_deps = None,
        extra_deps = None,
        extra_test_deps = None,
        test_data = None,
        test_env = None,
        crate_name = None,
        srcs = None,
        size = "small",
        visibility = None):
    """Single-crate boilerplate: lib + test + lint tests + manifest."""
    crate = name if crate_name == None else crate_name
    lib_srcs = srcs or ["src/lib.rs"]
    lib_deps = _crate_deps(
        deps,
        package_name = package_name,
    ) + (extra_deps or [])
    rust_library(
        name = name,
        srcs = lib_srcs,
        aliases = _aliases(
            package_name = package_name,
            normal = True,
        ),
        crate_name = crate,
        crate_root = "src/lib.rs",
        deps = lib_deps,
        visibility = visibility,
    )
    rust_test(
        name = name + "_test",
        size = size,
        aliases = _aliases(
            package_name = package_name,
            normal = True,
            normal_dev = True,
        ),
        crate = ":" + name,
        data = test_data or [],
        deps = _crate_deps(
            deps + (dev_deps or []),
            package_name = package_name,
        ) + (extra_deps or []) + (extra_test_deps or []),
        env = test_env or {},
        visibility = visibility,
    )
    rustfmt_test(
        name = name + "_fmt_test",
        size = size,
        targets = [":" + name],
    )
    rust_clippy_test(
        name = name + "_clippy_test",
        size = size,
        targets = [":" + name],
    )
    native.exports_files(
        ["Cargo.toml"],
        visibility = ["//visibility:public"],
    )
