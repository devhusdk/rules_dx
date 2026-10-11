"""Unit tests for the execution-requirements helper."""

load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_false", "expect_match", "expect_true", "starlark_test")
load(
    ":execution_requirements.bzl",
    "DX_FORBIDDEN_NO_REMOTE",
    "DX_HOST_EXECUTION_PLATFORM",
    "DX_NO_REMOTE_EXEC",
    "DX_REMOTE_CAPABILITIES",
    "REMOTE_QUALIFIED_STAGES",
    "RUNNER_REMOTE_QUALIFIED",
    "dx_action_remote_error",
    "dx_action_requirements",
    "dx_execution_requirements",
    "dx_has_forbidden_marker",
    "dx_is_local_only",
    "dx_remote_stage_error",
    "dx_remote_stage_key",
    "dx_remote_table_error",
)

_SHFMT_QUALIFIED = {"shfmt:format:host": True}

def execution_requirements_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "no stage records are enabled without remote-executor evidence",
                REMOTE_QUALIFIED_STAGES,
                {},
            ),
            expect_false(
                "shared runner stays unqualified so actions stay local-only",
                RUNNER_REMOTE_QUALIFIED,
            ),
            expect_equal(
                "helper returns the local-only marker",
                dx_execution_requirements(),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "marker key stays the frozen constant",
                DX_NO_REMOTE_EXEC,
                "no-remote-exec",
            ),
            expect_equal(
                "forbidden marker stays the cache-disabling key",
                DX_FORBIDDEN_NO_REMOTE,
                "no-remote",
            ),
            expect_true(
                "helper output reads as local-only",
                dx_is_local_only(dx_execution_requirements()),
            ),
            expect_false(
                "helper output carries no cache-disabling marker",
                dx_has_forbidden_marker(dx_execution_requirements()),
            ),
            expect_false(
                "empty requirements read as remote-capable, never local-only",
                dx_is_local_only({}),
            ),
            expect_true(
                "bare no-remote reads as forbidden",
                dx_has_forbidden_marker({"no-remote": "1"}),
            ),
            expect_equal(
                "host names the default execution platform",
                DX_HOST_EXECUTION_PLATFORM,
                "host",
            ),
            expect_contains(
                "format stays a remote-qualifiable capability",
                DX_REMOTE_CAPABILITIES,
                "format",
            ),
            expect_contains(
                "lint stays a remote-qualifiable capability",
                DX_REMOTE_CAPABILITIES,
                "lint",
            ),
            expect_equal(
                "stage key joins tool, capability and platform",
                dx_remote_stage_key("shfmt", "format", "host"),
                "shfmt:format:host",
            ),
            expect_equal(
                "enabled table passes schema validation",
                dx_remote_table_error(_SHFMT_QUALIFIED),
                "",
            ),
            expect_equal(
                "production table passes schema validation",
                dx_remote_table_error(REMOTE_QUALIFIED_STAGES),
                "",
            ),
            expect_match(
                "malformed record without separators fails validation",
                dx_remote_table_error({"shfmt": True}),
                "malformed",
            ),
            expect_match(
                "record with unknown capability fails validation",
                dx_remote_table_error({"shfmt:polish:host": True}),
                "capability",
            ),
            expect_equal(
                "fully qualified stages read as remote-capable",
                dx_action_requirements(["shfmt"], "format", "host", _SHFMT_QUALIFIED, True),
                {},
            ),
            expect_false(
                "remote-capable output reads as not local-only",
                dx_is_local_only(dx_action_requirements(["shfmt"], "format", "host", _SHFMT_QUALIFIED, True)),
            ),
            expect_false(
                "remote-capable output preserves caching",
                dx_has_forbidden_marker(dx_action_requirements(["shfmt"], "format", "host", _SHFMT_QUALIFIED, True)),
            ),
            expect_equal(
                "mixed stages stay local-only",
                dx_action_requirements(["shfmt", "taplo"], "format", "host", _SHFMT_QUALIFIED, True),
                {"no-remote-exec": "1"},
            ),
            expect_match(
                "mixed stages report the unqualified tool",
                dx_action_remote_error(["shfmt", "taplo"], "format", "host", _SHFMT_QUALIFIED, True),
                "taplo",
            ),
            expect_false(
                "mixed stages carry no cache-disabling marker",
                dx_has_forbidden_marker(dx_action_requirements(["shfmt", "taplo"], "format", "host", _SHFMT_QUALIFIED, True)),
            ),
            expect_equal(
                "unknown tool stays local-only",
                dx_action_requirements(["zzunknown"], "format", "host", _SHFMT_QUALIFIED, True),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "unknown capability stays local-only",
                dx_action_requirements(["shfmt"], "polish", "host", _SHFMT_QUALIFIED, True),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "unknown platform stays local-only",
                dx_action_requirements(["shfmt"], "format", "remote_x", _SHFMT_QUALIFIED, True),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "unqualified runner keeps qualified stages local-only",
                dx_action_requirements(["shfmt"], "format", "host", _SHFMT_QUALIFIED, False),
                {"no-remote-exec": "1"},
            ),
            expect_match(
                "unqualified runner reports the runner",
                dx_action_remote_error(["shfmt"], "format", "host", _SHFMT_QUALIFIED, False),
                "runner",
            ),
            expect_equal(
                "stageless action stays local-only",
                dx_action_requirements([], "format", "host", _SHFMT_QUALIFIED, True),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "non-canonical tool stays local-only",
                dx_action_requirements([""], "format", "host", _SHFMT_QUALIFIED, True),
                {"no-remote-exec": "1"},
            ),
            expect_match(
                "non-canonical tool reports the token fault",
                dx_remote_stage_error("", "format", "host", _SHFMT_QUALIFIED),
                "non-canonical",
            ),
            expect_equal(
                "production wiring stays local-only without enabled records",
                dx_action_requirements(["shfmt"], "format"),
                {"no-remote-exec": "1"},
            ),
        ],
    )
