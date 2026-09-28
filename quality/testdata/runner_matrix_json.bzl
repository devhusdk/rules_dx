"""Matrix JSON cases (split from runner_matrix_cases.bzl)."""

JSON_CASES = [
    {
        "name": "matrix_json_lint_pass",
        "srcs": [":real_clean.json"],
        "capability": "lint",
        "stages": ["biome;json;quality/testdata/real_clean.json"],
        "tool_names": ["biome"],
        "tool_binaries": ["@dx_tools//:biome"],
        "expected_file": ":matrix/matrix_json_lint_pass.expected.txt",
    },
    {
        "name": "matrix_json_lint_fail",
        "generated": {
            "matrix/json_dup.json": "{\"a\": 1, \"a\": 2}\n",
        },
        "capability": "lint",
        "stages": ["biome;json;matrix/json_dup.json"],
        "tool_names": ["biome"],
        "tool_binaries": ["@dx_tools//:biome"],
        "expected_file": ":matrix/matrix_json_lint_fail.expected.txt",
    },
    {
        "name": "matrix_json_format_pass",
        "srcs": [":real_clean.json"],
        "capability": "format",
        "stages": ["prettier;json;quality/testdata/real_clean.json"],
        "tool_names": ["prettier"],
        "tool_binaries": ["//quality/tools/javascript/bin:prettier"],
        "expected_file": ":matrix/matrix_json_format_pass.expected.txt",
    },
    {
        "name": "matrix_json_format_fail",
        "srcs": [":real_dirty.json"],
        "capability": "format",
        "stages": ["prettier;json;quality/testdata/real_dirty.json"],
        "tool_names": ["prettier"],
        "tool_binaries": ["//quality/tools/javascript/bin:prettier"],
        "expected_file": ":matrix/matrix_json_format_fail.expected.txt",
    },
]
