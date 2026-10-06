"""Repository crate helper; loads the rules_dx @crates hub."""

load("@crates//:crates.bzl", _aliases = "aliases", _crate_deps = "crate_deps")
load(":defs.bzl", "rust_clippy_test", "rust_library", "rust_test", "rustfmt_test")

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
