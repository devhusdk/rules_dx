"""Matrix JSON cases."""

load(":runner_matrix_tests.bzl", "BIOME_BINARY", "matrix_case")

JSON_CASES = [
    matrix_case(
        name = "matrix_json_lint_pass",
        capability = "lint",
        tool = "biome",
        binary = BIOME_BINARY,
        source_class = "json",
        src = ":real_clean.json",
        platforms = ["linux_x86_64"],
    ),
    matrix_case(
        name = "matrix_json_lint_fail",
        capability = "lint",
        tool = "biome",
        binary = BIOME_BINARY,
        source_class = "json",
        generated = {
            "matrix/json_dup.json": "{\"a\": 1, \"a\": 2}\n",
        },
        platforms = ["linux_x86_64"],
    ),
    matrix_case(
        name = "matrix_json_format_pass",
        capability = "format",
        tool = "biome",
        binary = BIOME_BINARY,
        source_class = "json",
        generated = {
            "matrix/json_clean.json": "{\n\t\"a\": 1\n}\n",
        },
        platforms = ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"],
    ),
]
