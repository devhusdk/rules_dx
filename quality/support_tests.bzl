"""Unit tests for explicit tool support records."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "real_supported_classes")
load(":support.bzl", "TEXT_KEEP_SORTED_SUPPORT", "is_support_tool", "support_artifact_error", "support_cli_error", "support_record_error", "support_schema_error", "support_state", "support_wiring_error")

def support_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "support schema validates",
                support_schema_error(),
                "",
            ),
            expect_equal(
                "keep_sorted adapter support stays text lint",
                real_supported_classes("keep_sorted", "lint"),
                ["text"],
            ),
            expect_equal(
                "keep_sorted is a support tool with known state",
                [is_support_tool("keep_sorted"), support_state("keep_sorted")],
                [True, "known"],
            ),
            expect_equal(
                "unknown tools stay outside support",
                [is_support_tool("ruff"), support_state("ruff")],
                [False, "unknown"],
            ),
            expect_equal(
                "missing aspect wiring is an actionable failure",
                support_wiring_error([]),
                "support: keep_sorted lint is known but has no wired aspect (missing real_text_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "wired keep_sorted clears the aspect gap",
                support_wiring_error(["keep_sorted"]),
                "",
            ),
            expect_equal(
                "missing CLI aspect is an actionable failure",
                support_cli_error(["//quality:real_aspects.bzl%real_lint_aspect"]),
                "support: keep_sorted lint is known but has no CLI lint aspect (missing real_text_lint_aspect)",
            ),
            expect_equal(
                "CLI text aspect clears the CLI gap",
                support_cli_error(["//quality:real_aspects.bzl%real_text_lint_aspect"]),
                "",
            ),
            expect_equal(
                "missing artifact is an actionable failure",
                support_artifact_error(),
                "support: keep_sorted lint is known but has no pinned artifact (quality/artifacts/keep_sorted.*.bzl is missing)",
            ),
            expect_equal(
                "unknown tool is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "not_a_tool"}),
                "support: unknown tool 'not_a_tool': not in the real adapter registry",
            ),
            expect_equal(
                "unknown capability is rejected",
                support_record_error({"artifact": "missing", "capability": "audit", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted"}),
                "support: tool 'keep_sorted' has no 'audit' capability",
            ),
            expect_equal(
                "unclassified class is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["not_a_class"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted"}),
                "support: tool 'keep_sorted' names unclassified class 'not_a_class'",
            ),
            expect_equal(
                "wrong family is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "python", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted"}),
                "support: class 'text' belongs to family 'text', not 'python'",
            ),
            expect_equal(
                "class outside adapter support is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["python"], "context": "owned", "execution": "direct", "family": "python", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted"}),
                "support: class 'python' is outside 'keep_sorted lint' adapter support",
            ),
            expect_equal(
                "bad execution is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "fork", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted"}),
                "support: tool 'keep_sorted' needs execution 'direct' or 'delegated', got 'fork'",
            ),
            expect_equal(
                "bad fix is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "rewrite", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted"}),
                "support: tool 'keep_sorted' needs fix 'supported' or 'check_only', got 'rewrite'",
            ),
            expect_equal(
                "bad state is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "maybe", "tool": "keep_sorted"}),
                "support: tool 'keep_sorted' needs state 'known', 'supported' or 'qualified', got 'maybe'",
            ),
            expect_equal(
                "record keeps the migrated platforms and fix",
                [TEXT_KEEP_SORTED_SUPPORT["platforms"], TEXT_KEEP_SORTED_SUPPORT["fix"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"], "supported"],
            ),
        ],
    )
