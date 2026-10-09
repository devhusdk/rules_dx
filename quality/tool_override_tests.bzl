"""Unit tests for consumer-owned tool overrides."""

load("//libs/starlark:defs.bzl", "expect_equal", "expect_true", "starlark_test")
load(":tool_override.bzl", "collect_tool_overrides", "tool_override_error")

_HINTS = [
    struct(tool_id = "biome", version = "2.3.4-dx-override"),
    struct(tool_id = "ruff", version = "0.16.7-dx-override"),
]

def tool_override_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "tool_override_error accepts a known tool with version and executable",
                tool_override_error("ruff", "0.16.7-dx-override", True),
                "",
            ),
            expect_true(
                "tool_override_error rejects an unknown tool",
                tool_override_error("frobnicate", "1.0", True) != "",
            ),
            expect_equal(
                "tool_override_error rejects an empty version",
                tool_override_error("ruff", "", True),
                "tool_override (ruff): version is required; declare the replacement build identity",
            ),
            expect_equal(
                "tool_override_error rejects a missing executable",
                tool_override_error("ruff", "0.16.7-dx-override", False),
                "tool_override (ruff): tool is required; declare the replacement executable",
            ),
            expect_equal(
                "collect_tool_overrides keeps the staged tools",
                collect_tool_overrides(_HINTS, ["ruff"], "//q:t"),
                {"ruff": _HINTS[1]},
            ),
            expect_equal(
                "collect_tool_overrides ignores hints for unstaged tools",
                collect_tool_overrides(_HINTS, ["taplo"], "//q:t"),
                {},
            ),
            expect_equal(
                "collect_tool_overrides accepts an empty hint list",
                collect_tool_overrides([], ["ruff"], "//q:t"),
                {},
            ),
        ],
    )
