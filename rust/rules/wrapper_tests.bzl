"""Wrapper conformance tests."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//quality:sources.bzl", "KNOWN_SEMANTIC_FILE_CLASSES", "RUST")

def dx_wrapper_registry_tests(name):
    """Instantiates Rust wrapper registry tests."""
    dx_wrapper_contract_tests(
        name,
        include_forwarder_checks = True,
        extra_checks = [
            expect_equal("RUST class id", RUST, "rust"),
            expect_equal("rust is a known class", RUST in KNOWN_SEMANTIC_FILE_CLASSES, True),
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
