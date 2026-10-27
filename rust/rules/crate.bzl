"""Repository crate helper; loads the rules_dx @crates hub."""

load("@crates//:crates.bzl", _aliases = "aliases", _crate_deps = "crate_deps")
load(":defs.bzl", "rust_clippy_test", "rust_library", "rust_test", "rustfmt_test")

def dx_rust_crate_root(srcs):
    """Returns src/lib.rs when it is a source, else None."""
    if srcs == None or "src/lib.rs" in srcs:
        return "src/lib.rs"
    return None

def _test_crate_root(srcs):
    """Returns the library crate root for a self-contained unit test."""
    root = dx_rust_crate_root(srcs)
    if root != None:
        return root
    if len(srcs) == 1:
        return srcs[0]
    fail("dx_rust_crate test_srcs needs src/lib.rs or a single library source")

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
        test_srcs = None,
        compile_data = None,
        size = "small",
        testonly = None,
        visibility = None):
    """Single-crate boilerplate: lib + test + lint tests + manifest."""
    crate = name if crate_name == None else crate_name
    lib_srcs = srcs or ["src/lib.rs"]
    if test_srcs and srcs == None:
        fail("dx_rust_crate test_srcs needs srcs")
    overlap = [src for src in (test_srcs or []) if src in lib_srcs]
    if overlap:
        fail("dx_rust_crate srcs and test_srcs overlap: {}".format(overlap))
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
        compile_data = compile_data or [],
        crate_name = crate,
        crate_root = dx_rust_crate_root(srcs),
        deps = lib_deps,
        testonly = testonly,
        visibility = visibility,
    )
    test_aliases = _aliases(
        package_name = package_name,
        normal = True,
        normal_dev = True,
    )
    test_deps = _crate_deps(
        deps + (dev_deps or []),
        package_name = package_name,
    ) + (extra_deps or []) + (extra_test_deps or [])
    if test_srcs:
        rust_test(
            name = name + "_test",
            size = size,
            srcs = lib_srcs + test_srcs,
            aliases = test_aliases,
            compile_data = compile_data or [],
            crate_name = crate,
            crate_root = _test_crate_root(lib_srcs),
            data = test_data or [],
            deps = test_deps,
            env = test_env or {},
            testonly = testonly,
            visibility = visibility,
        )
    else:
        rust_test(
            name = name + "_test",
            size = size,
            aliases = test_aliases,
            crate = ":" + name,
            data = test_data or [],
            deps = test_deps,
            env = test_env or {},
            testonly = testonly,
            visibility = visibility,
        )
    rustfmt_test(
        name = name + "_fmt_test",
        size = size,
        targets = [":" + name, ":" + name + "_test"] if test_srcs else [":" + name],
        testonly = testonly,
    )
    rust_clippy_test(
        name = name + "_clippy_test",
        size = size,
        targets = [":" + name],
        testonly = testonly,
    )
    native.exports_files(
        ["Cargo.toml"],
        visibility = ["//visibility:public"],
    )
