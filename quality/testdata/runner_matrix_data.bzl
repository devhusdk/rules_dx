"""Matrix data cases (split from runner_matrix_cases.bzl)."""

DATA_CASES = [
    {
        "name": "matrix_starlark_lint_pass",
        "srcs": [":real_clean.bzl"],
        "capability": "lint",
        "stages": ["buildifier;starlark;quality/testdata/real_clean.bzl"],
        "tool_names": ["buildifier"],
        "tool_binaries": ["@dx_tools//:buildifier"],
        "expected_file": ":matrix/matrix_starlark_lint_pass.expected.txt",
    },
    {
        "name": "matrix_starlark_lint_fail",
        "generated": {
            "matrix/starlark_dirty.bzl": "x=1\n",
        },
        "capability": "lint",
        "stages": ["buildifier;starlark;matrix/starlark_dirty.bzl"],
        "tool_names": ["buildifier"],
        "tool_binaries": ["@dx_tools//:buildifier"],
        "expected_file": ":matrix/matrix_starlark_lint_fail.expected.txt",
    },
    {
        "name": "matrix_starlark_format_pass",
        "srcs": [":real_clean.bzl"],
        "capability": "format",
        "stages": ["buildifier;starlark;quality/testdata/real_clean.bzl"],
        "tool_names": ["buildifier"],
        "tool_binaries": ["@dx_tools//:buildifier"],
        "expected_file": ":matrix/matrix_starlark_format_pass.expected.txt",
    },
    {
        "name": "matrix_starlark_format_fail",
        "generated": {
            "matrix/starlark_dirty.bzl": "x=1\n",
        },
        "capability": "format",
        "stages": ["buildifier;starlark;matrix/starlark_dirty.bzl"],
        "tool_names": ["buildifier"],
        "tool_binaries": ["@dx_tools//:buildifier"],
        "expected_file": ":matrix/matrix_starlark_format_fail.expected.txt",
    },
    {
        "name": "matrix_toml_lint_pass",
        "srcs": [":real_clean.toml"],
        "capability": "lint",
        "stages": ["taplo;toml;quality/testdata/real_clean.toml"],
        "tool_names": ["taplo"],
        "tool_binaries": ["@dx_tools//:taplo"],
        "expected_file": ":matrix/matrix_toml_lint_pass.expected.txt",
    },
    {
        "name": "matrix_toml_lint_fail",
        "generated": {
            "matrix/toml_dirty.toml": "a=1\n",
        },
        "capability": "lint",
        "stages": ["taplo;toml;matrix/toml_dirty.toml"],
        "tool_names": ["taplo"],
        "tool_binaries": ["@dx_tools//:taplo"],
        "expected_file": ":matrix/matrix_toml_lint_fail.expected.txt",
    },
    {
        "name": "matrix_toml_format_pass",
        "srcs": [":real_clean.toml"],
        "capability": "format",
        "stages": ["taplo;toml;quality/testdata/real_clean.toml"],
        "tool_names": ["taplo"],
        "tool_binaries": ["@dx_tools//:taplo"],
        "expected_file": ":matrix/matrix_toml_format_pass.expected.txt",
    },
    {
        "name": "matrix_toml_format_fail",
        "generated": {
            "matrix/toml_dirty.toml": "a=1\n",
        },
        "capability": "format",
        "stages": ["taplo;toml;matrix/toml_dirty.toml"],
        "tool_names": ["taplo"],
        "tool_binaries": ["@dx_tools//:taplo"],
        "expected_file": ":matrix/matrix_toml_format_fail.expected.txt",
    },
]
