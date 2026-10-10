"""Behavioral-matrix mapping validation."""

load("//libs/starlark:defs.bzl", "starlark_test")

_MATRIX = "//libs/starlark:behavioral_matrix.md"

def matrix_validation_tests(name):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            _MATRIX: "matrix-item: expect_equal\nmatrix-item: expect-true-false\nmatrix-item: expect-contains\nmatrix-item: expect-match\nmatrix-item: starlark_test-facade\nmatrix-item: load-mode\nmatrix-item: unit-mode\nmatrix-item: analysis-mode\nmatrix-item: execution-mode\nmatrix-item: harness-execution-mode\nmatrix-item: dx-subject-info\nmatrix-item: aspect-subjects\nmatrix-item: configuration-subjects\nmatrix-item: failure-rendering\nmatrix-item: mode-validation\nmatrix-item: tested-stack",
            "//libs/starlark/tests/negative:negative_tests.bzl": "wrong_phase_demo",
            "//libs/starlark/tests:analysis_tests.bzl": "expected_observations",
            "//libs/starlark/tests:arithmetic_tests.bzl": "expect_equal",
            "//libs/starlark/tests:aspect_tests.bzl": "aspect_subject_tests",
            "//libs/starlark/tests:config_tests.bzl": "config_subject_tests",
            "//libs/starlark/tests:execution_tests.bzl": "file_checks",
            "//libs/starlark/tests:matcher_tests.bzl": "expect_contains",
            "//libs/starlark/tests:subject.bzl": "DxSubjectInfo",
            "//libs/starlark/tests/fixtures/starlark_futures:aspect_subjects.bzl": "aspect_leaf",
            "//libs/starlark/tests/fixtures/starlark_futures:config_subjects.bzl": "config_leaf",
            "//libs/starlark/tests/fixtures/starlark_futures:matchers.bzl": "def greet_report",
            "//libs/starlark:defs.bzl": "def starlark_test\ndef expect_equal\ndef expect_true\ndef expect_false\ndef expect_contains\ndef expect_match\nDxAspectInfo\nDxConfigInfo\ndx_aspect_note\naspect_field\nconfig_field\ndef _load_test_impl\ndef _unit_test_impl\ndef _analysis_test_impl\ndef _execution_test_impl\ncheck_true\ncheck_contains\ncheck_match\nmust be empty",
            "//libs/testing:tested_stack.bzl": "schema_version",
        },
    )
