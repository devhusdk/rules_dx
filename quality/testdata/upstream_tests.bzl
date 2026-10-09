"""Upstream ownership evidence tests."""

load("//libs/starlark:defs.bzl", "starlark_test")

EXPECTED_OBSERVATIONS = """subject //quality/testdata:fixture_upstream_cc_empty_ownership_subject
field label=//quality/testdata:fixture_upstream_cc_empty
field owned=(empty)
field status=upstream
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_upstream_cc_empty_ownership_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_upstream_cc_ownership_subject
field label=//quality/testdata:fixture_upstream_cc
field owned=c:upstream_greeting.h;cpp:upstream_greeting.cc
field status=upstream
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_upstream_cc_ownership_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_upstream_rust_clean_ownership_subject
field label=//quality/testdata:fixture_upstream_rust_clean
field owned=rust:upstream_clean.rs
field status=upstream
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_upstream_rust_clean_ownership_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_upstream_rust_configured_ownership_subject
field label=//quality/testdata:fixture_upstream_rust_configured
field owned=rust:upstream_clean.rs
field status=upstream
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_upstream_rust_configured_ownership_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_upstream_rust_dirty_ownership_subject
field label=//quality/testdata:fixture_upstream_rust_dirty
field owned=rust:upstream_dirty.rs
field status=upstream
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_upstream_rust_dirty_ownership_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_upstream_rust_generated_ownership_subject
field label=//quality/testdata:fixture_upstream_rust_generated
field owned=rust:generated_upstream.rs
field status=upstream
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_upstream_rust_generated_ownership_subject
aspect_field transitive_count=0
subject //quality/testdata:plain_ownership_subject
field label=//quality/testdata:plain
field owned=(none)
field status=unsupported
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:plain_ownership_subject
aspect_field transitive_count=0
subject //quality/testdata:wrapper_ownership_subject
field label=//quality/testdata:fixture_real_rust
field owned=rust:clean.rs
field status=wrapper
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:wrapper_ownership_subject
aspect_field transitive_count=0"""

def upstream_ownership_fixture_tests(name, subjects):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = subjects,
        expected_observations = EXPECTED_OBSERVATIONS,
    )
