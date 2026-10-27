"""Real typecheck aspect evidence tests."""

load("//libs/starlark:defs.bzl", "starlark_test")

EXPECTED_TYPECHECK_OBSERVATIONS = """subject //quality/testdata:fixture_real_python_no_typecheck_subject
field dx_count=0
field dx_results=(none)
field has_quality_sources=True
field label=//quality/testdata:fixture_real_python_no_typecheck
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_real_python_no_typecheck_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_real_python_typecheck_subject
field dx_count=1
field dx_results=fixture_real_python-real-typecheck.pb
field has_quality_sources=True
field label=//quality/testdata:fixture_real_python
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_real_python_typecheck_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_real_rust_test_crate_typecheck_subject
field dx_count=1
field dx_results=fake_format_test-real-typecheck-rust.pb
field has_quality_sources=True
field label=//quality/testdata:fake_format_test
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_real_rust_test_crate_typecheck_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_real_rust_typecheck_clean_subject
field dx_count=1
field dx_results=fixture_real_rust_typecheck_clean-real-typecheck-rust.pb
field has_quality_sources=True
field label=//quality/testdata:fixture_real_rust_typecheck_clean
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_real_rust_typecheck_clean_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_real_rust_typecheck_no_typecheck_subject
field dx_count=0
field dx_results=(none)
field has_quality_sources=True
field label=//quality/testdata:fixture_real_rust_typecheck_no_typecheck
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_real_rust_typecheck_no_typecheck_subject
aspect_field transitive_count=0
subject //quality/testdata:fixture_real_starlark_typecheck_subject
field dx_count=0
field dx_results=(none)
field has_quality_sources=True
field label=//quality/testdata:fixture_real_starlark
aspect_field aspect_seen=True
aspect_field field_count=4
aspect_field has_subject=True
aspect_field subject_label=//quality/testdata:fixture_real_starlark_typecheck_subject
aspect_field transitive_count=0"""

def real_typecheck_fixture_tests(name, subjects):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = subjects,
        expected_observations = EXPECTED_TYPECHECK_OBSERVATIONS,
    )
