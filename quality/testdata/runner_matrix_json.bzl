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
        "generated": {
            "matrix/json_clean.json": "{\n\t\"a\": 1\n}\n",
        },
        "capability": "format",
        "stages": ["biome;json;matrix/json_clean.json"],
        "tool_names": ["biome"],
        "tool_binaries": ["@dx_tools//:biome"],
        "expected_file": ":matrix/matrix_json_format_pass.expected.txt",
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
    },
]
