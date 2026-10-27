"""Wrapper conformance tests."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_rust//rust:defs.bzl", "rust_common")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//quality:sources.bzl", "KNOWN_SEMANTIC_FILE_CLASSES", "RUST")
load(":crate.bzl", "dx_rust_crate_root")

def dx_wrapper_registry_tests(name):
    """Instantiates Rust wrapper registry tests."""
    dx_wrapper_contract_tests(
        name,
        include_forwarder_checks = True,
        extra_checks = [
            expect_equal("RUST class id", RUST, "rust"),
            expect_equal("rust is a known class", RUST in KNOWN_SEMANTIC_FILE_CLASSES, True),
            expect_equal("crate root default", dx_rust_crate_root(None), "src/lib.rs"),
            expect_equal("crate root multi-file", dx_rust_crate_root(["src/a.rs", "src/lib.rs"]), "src/lib.rs"),
            expect_equal("crate root missing", dx_rust_crate_root(["lib.rs"]), None),
        ],
    )

EXPECTED_OBSERVATIONS = """subject //rust/tests/fixtures/hello:hello_cdylib_subject
file hello_cdylib_subject.txt
field cargo_tool=cargo
field cc_linker_inputs=2
field clippy_markers=hello_cdylib.clippy.ok,hello_derive.clippy.ok,hello_staticlib.clippy.ok
field clippy_tool=clippy-driver
field crate_edition=2021
field crate_is_test=False
field crate_name=hello_cdylib
field crate_owner=//rust/tests/fixtures/hello:hello_cdylib_upstream
field crate_root=cdylib.rs
field crate_srcs=cdylib.rs
field crate_type=cdylib
field direct_sources=rust:cdylib.rs
field fmt_markers=hello_cdylib.rustfmt.ok,hello_derive.rustfmt.ok,hello_staticlib.rustfmt.ok
field preserved_cc_inputs=True
field preserved_deps=True
field preserved_edition=True
field preserved_name=True
field preserved_root=True
field preserved_srcs=True
field preserved_type=True
field rustc_tool=rustc
field rustfmt_tool=rustfmt
field tools_pinned=True
field upstream=//rust/tests/fixtures/hello:hello_cdylib_upstream
field upstream_has_instrumented_files=True
field upstream_has_quality_sources=False
field wrapper=//rust/tests/fixtures/hello:hello_cdylib
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=29
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:hello_cdylib_subject
aspect_field transitive_count=0
subject //rust/tests/fixtures/hello:hello_derive_subject
file hello_derive_subject.txt
field cargo_tool=cargo
field clippy_markers=hello_cdylib.clippy.ok,hello_derive.clippy.ok,hello_staticlib.clippy.ok
field clippy_tool=clippy-driver
field crate_edition=2021
field crate_is_test=False
field crate_name=hello_derive
field crate_owner=//rust/tests/fixtures/hello:hello_derive_upstream
field crate_root=derive.rs
field crate_srcs=derive.rs
field crate_type=proc-macro
field direct_sources=rust:derive.rs
field fmt_markers=hello_cdylib.rustfmt.ok,hello_derive.rustfmt.ok,hello_staticlib.rustfmt.ok
field preserved_deps=True
field preserved_edition=True
field preserved_name=True
field preserved_root=True
field preserved_srcs=True
field preserved_type=True
field rustc_tool=rustc
field rustfmt_tool=rustfmt
field tools_pinned=True
field upstream=//rust/tests/fixtures/hello:hello_derive_upstream
field upstream_has_instrumented_files=True
field upstream_has_quality_sources=False
field wrapper=//rust/tests/fixtures/hello:hello_derive
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=27
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:hello_derive_subject
aspect_field transitive_count=0
subject //rust/tests/fixtures/hello:hello_lib_subject
file hello_lib_subject.txt
field cargo_tool=cargo
field clippy_markers=hello.clippy.ok,hello_lib.clippy.ok
field clippy_tool=clippy-driver
field crate_edition=2021
field crate_is_test=False
field crate_name=hello
field crate_owner=//rust/tests/fixtures/hello:hello_lib_upstream
field crate_root=lib.rs
field crate_srcs=lib.rs
field crate_type=rlib
field direct_sources=rust:lib.rs
field fmt_markers=hello.rustfmt.ok,hello_lib.rustfmt.ok
field preserved_deps=True
field preserved_edition=True
field preserved_name=True
field preserved_root=True
field preserved_srcs=True
field preserved_type=True
field rustc_tool=rustc
field rustfmt_tool=rustfmt
field tools_pinned=True
field upstream=//rust/tests/fixtures/hello:hello_lib_upstream
field upstream_has_instrumented_files=True
field upstream_has_quality_sources=False
field wrapper=//rust/tests/fixtures/hello:hello_lib
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=27
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:hello_lib_subject
aspect_field transitive_count=0
subject //rust/tests/fixtures/hello:hello_staticlib_subject
file hello_staticlib_subject.txt
field cargo_tool=cargo
field cc_linker_inputs=2
field clippy_markers=hello_cdylib.clippy.ok,hello_derive.clippy.ok,hello_staticlib.clippy.ok
field clippy_tool=clippy-driver
field crate_edition=2021
field crate_is_test=False
field crate_name=hello_staticlib
field crate_owner=//rust/tests/fixtures/hello:hello_staticlib_upstream
field crate_root=staticlib.rs
field crate_srcs=staticlib.rs
field crate_type=staticlib
field direct_sources=rust:staticlib.rs
field fmt_markers=hello_cdylib.rustfmt.ok,hello_derive.rustfmt.ok,hello_staticlib.rustfmt.ok
field preserved_cc_inputs=True
field preserved_deps=True
field preserved_edition=True
field preserved_name=True
field preserved_root=True
field preserved_srcs=True
field preserved_type=True
field rustc_tool=rustc
field rustfmt_tool=rustfmt
field tools_pinned=True
field upstream=//rust/tests/fixtures/hello:hello_staticlib_upstream
field upstream_has_instrumented_files=True
field upstream_has_quality_sources=False
field wrapper=//rust/tests/fixtures/hello:hello_staticlib
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=29
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:hello_staticlib_subject
aspect_field transitive_count=0
subject //rust/tests/fixtures/hello:hello_subject
file hello_subject.txt
field cargo_tool=cargo
field clippy_markers=hello.clippy.ok,hello_lib.clippy.ok
field clippy_tool=clippy-driver
field crate_edition=2021
field crate_is_test=False
field crate_name=hello
field crate_owner=//rust/tests/fixtures/hello:hello_upstream
field crate_root=main.rs
field crate_srcs=main.rs
field crate_type=bin
field direct_sources=rust:main.rs
field fmt_markers=hello.rustfmt.ok,hello_lib.rustfmt.ok
field preserved_deps=True
field preserved_edition=True
field preserved_name=True
field preserved_root=True
field preserved_srcs=True
field preserved_type=True
field rustc_tool=rustc
field rustfmt_tool=rustfmt
field tools_pinned=True
field upstream=//rust/tests/fixtures/hello:hello_upstream
field upstream_has_instrumented_files=True
field upstream_has_quality_sources=False
field wrapper=//rust/tests/fixtures/hello:hello
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=27
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:hello_subject
aspect_field transitive_count=0
subject //rust/tests/fixtures/hello:hello_test_subject
file hello_test_subject.txt
field cargo_tool=cargo
field clippy_markers=hello.clippy.ok,hello_lib.clippy.ok
field clippy_tool=clippy-driver
field crate_edition=2021
field crate_is_test=True
field crate_name=hello
field crate_owner=//rust/tests/fixtures/hello:hello_test_upstream
field crate_root=lib.rs
field crate_srcs=lib.rs
field crate_type=bin
field direct_sources=(none)
field fmt_markers=hello.rustfmt.ok,hello_lib.rustfmt.ok
field preserved_deps=True
field preserved_edition=True
field preserved_name=True
field preserved_root=True
field preserved_srcs=True
field preserved_type=True
field rustc_tool=rustc
field rustfmt_tool=rustfmt
field tools_pinned=True
field upstream=//rust/tests/fixtures/hello:hello_test_upstream
field upstream_has_instrumented_files=True
field upstream_has_quality_sources=False
field wrapper=//rust/tests/fixtures/hello:hello_test
field wrapper_has_instrumented_files=True
field wrapper_has_quality_sources=True
aspect_field aspect_seen=True
aspect_field field_count=27
aspect_field has_subject=True
aspect_field subject_label=//rust/tests/fixtures/hello:hello_test_subject
aspect_field transitive_count=0"""

def dx_wrapper_conformance_tests(name, subjects):
    """Instantiates Rust wrapper conformance tests."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = subjects,
        expected_observations = EXPECTED_OBSERVATIONS,
    )

def _native_coverage_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    metadata = [f.short_path for f in target[InstrumentedFilesInfo].metadata_files.to_list()]
    asserts.true(
        env,
        ctx.attr.objects_list in metadata,
        "missing " + ctx.attr.objects_list + " in " + str(metadata),
    )
    run_env = target[RunEnvironmentInfo].environment
    script = run_env.get("CC_CODE_COVERAGE_SCRIPT", "")
    asserts.true(env, script.removesuffix(".exe").endswith("/tools/coverage/collect/collect_native"), "CC_CODE_COVERAGE_SCRIPT=" + script)
    asserts.true(env, run_env.get("LLVM_COV", "").removesuffix(".exe").endswith("llvm-cov"), "LLVM_COV=" + run_env.get("LLVM_COV", ""))
    asserts.true(env, run_env.get("LLVM_PROFDATA", "").removesuffix(".exe").endswith("llvm-profdata"), "LLVM_PROFDATA=" + run_env.get("LLVM_PROFDATA", ""))
    return analysistest.end(env)

_native_coverage_test = analysistest.make(
    _native_coverage_impl,
    attrs = {"objects_list": attr.string(mandatory = True)},
    config_settings = {
        "//command_line_option:collect_code_coverage": True,
        "//command_line_option:experimental_generate_llvm_lcov": True,
        "//command_line_option:features": ["-coverage", "-gcc_coverage_map_format"],
    },
)

def dx_native_coverage_tests(name, target, objects_list):
    """Asserts a Rust test exposes its native data binary to the dx coverage collector."""
    _native_coverage_test(
        name = name,
        size = "small",
        target_under_test = target,
        objects_list = objects_list,
    )

def _default_crate_name_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    if rust_common.test_crate_info in target:
        name = target[rust_common.test_crate_info].crate.name
    else:
        name = target[rust_common.crate_info].name
    asserts.equals(env, ctx.attr.expected, name)
    return analysistest.end(env)

_default_crate_name_test = analysistest.make(
    _default_crate_name_impl,
    attrs = {"expected": attr.string(mandatory = True)},
)

def dx_default_crate_name_tests(name, target, expected):
    """Asserts a Rust target defaults its crate name to the rewritten target name."""
    _default_crate_name_test(
        name = name,
        size = "small",
        target_under_test = target,
        expected = expected,
    )

def _crate_srcs(target):
    if rust_common.test_crate_info in target:
        crate = target[rust_common.test_crate_info].crate
    else:
        crate = target[rust_common.crate_info]
    return crate.name, [src.short_path for src in crate.srcs.to_list()]

def _test_srcs_split_impl(ctx):
    env = analysistest.begin(ctx)
    test_name, test_srcs = _crate_srcs(analysistest.target_under_test(env))
    lib_name, lib_srcs = _crate_srcs(ctx.attr.lib)
    asserts.equals(env, lib_name, test_name)
    asserts.false(env, ctx.attr.test_src in lib_srcs, "{} is in the library srcs".format(ctx.attr.test_src))
    asserts.true(env, ctx.attr.test_src in test_srcs, "{} is missing from the test srcs".format(ctx.attr.test_src))
    for src in lib_srcs:
        asserts.true(env, src in test_srcs, "{} is missing from the test srcs".format(src))
    return analysistest.end(env)

_test_srcs_split_test = analysistest.make(
    _test_srcs_split_impl,
    attrs = {
        "lib": attr.label(mandatory = True),
        "test_src": attr.string(mandatory = True),
    },
)

def dx_test_srcs_split_tests(name, lib, test, test_src):
    """Asserts a dx_rust_crate test_srcs file builds only into the unit test."""
    _test_srcs_split_test(
        name = name,
        size = "small",
        target_under_test = test,
        lib = lib,
        test_src = test_src,
    )

def _browser_cell_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    run_env = target[RunEnvironmentInfo].environment
    asserts.equals(env, "firefox", run_env.get("BROWSER_TYPE", ""))
    return analysistest.end(env)

_browser_cell_test = analysistest.make(
    _browser_cell_impl,
    config_settings = {
        str(Label("@rules_rust_wasm_bindgen//settings:test_browser")): "chrome",
    },
)

def dx_browser_cell_tests(name, target):
    """Asserts a browser test runs the qualified cell's browser over the command line flag."""
    _browser_cell_test(
        name = name,
        size = "small",
        target_under_test = target,
        target_compatible_with = [
            "@platforms//cpu:x86_64",
            "@platforms//os:linux",
        ],
    )
