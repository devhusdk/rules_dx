"""Unit tests for the execution-requirements helper."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("//core:defs.bzl", "expect_equal", "expect_false", "expect_match", "expect_true", "starlark_test")
load(":execution_requirements.bzl", "DX_FORBIDDEN_NO_REMOTE", "DX_NO_REMOTE_EXEC", "REMOTE_QUALIFIED", "SHFMT_FORMAT_REMOTE", "dx_has_forbidden_marker", "dx_is_local_only", "dx_is_remote_qualified", "dx_remote_platforms", "dx_stage_execution_requirements", "remote_platform_error", "remote_record_error")

visibility("private")

def _artifact_platforms():
    """Returns the pinned artifact platforms keyed by tool."""
    by_tool = {}
    for artifact in [a for t in DATA["dx_quality_tools"].values() for a in t.values()]:
        by_tool.setdefault(artifact["tool"], []).append(artifact["os"] + "_" + artifact["cpu"])
    return by_tool

def _remote_record(tool = "shfmt", capability = "format", closure = "declared", platforms = ["linux_x86_64"], runner = "hermetic", state = "qualified"):
    return {
        "capability": capability,
        "closure": closure,
        "platforms": platforms,
        "runner": runner,
        "state": state,
        "tool": tool,
    }

def execution_requirements_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
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
                dx_is_local_only({DX_NO_REMOTE_EXEC: "1"}),
            ),
            expect_false(
                "helper output carries no cache-disabling marker",
                dx_has_forbidden_marker({DX_NO_REMOTE_EXEC: "1"}),
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
                "shfmt format remote record validates",
                remote_record_error(SHFMT_FORMAT_REMOTE),
                "",
            ),
            expect_equal(
                "shfmt format is remote-qualified on four execution platforms",
                dx_remote_platforms("shfmt", "format"),
                ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"],
            ),
            expect_true(
                "shfmt format reads as remote-qualified",
                dx_is_remote_qualified("shfmt", "format"),
            ),
            expect_false(
                "shfmt lint is not remote-qualified",
                dx_is_remote_qualified("shfmt", "lint"),
            ),
            expect_false(
                "ruff format stays unqualified",
                dx_is_remote_qualified("ruff", "format"),
            ),
            expect_equal(
                "shfmt-only format stages lose the local-only marker",
                dx_stage_execution_requirements(["shfmt"], "format"),
                {},
            ),
            expect_equal(
                "unqualified tool stages stay local-only",
                dx_stage_execution_requirements(["ruff"], "format"),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "mixed qualified and unqualified stages stay local-only",
                dx_stage_execution_requirements(["shfmt", "ruff"], "format"),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "qualified tool under another capability stays local-only",
                dx_stage_execution_requirements(["shfmt"], "lint"),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "unknown tool stages stay local-only",
                dx_stage_execution_requirements(["not_a_tool"], "format"),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "empty stages stay local-only",
                dx_stage_execution_requirements([], "format"),
                {"no-remote-exec": "1"},
            ),
            expect_false(
                "qualified stage output carries no cache-disabling marker",
                dx_has_forbidden_marker(dx_stage_execution_requirements(["shfmt"], "format")),
            ),
            expect_equal(
                "pinned artifacts cover every qualified shfmt platform",
                remote_platform_error(SHFMT_FORMAT_REMOTE, _artifact_platforms()),
                "",
            ),
            expect_equal(
                "the text lint stage runs remotely once keep-sorted and typos both qualify",
                dx_stage_execution_requirements(["keep_sorted", "typos"], "lint"),
                {},
            ),
            expect_equal(
                "taplo is qualified for format only",
                [dx_is_remote_qualified("taplo", "format"), dx_is_remote_qualified("taplo", "lint")],
                [True, False],
            ),
            expect_equal(
                "the single-tool format stages run remotely",
                [dx_stage_execution_requirements([tool], "format") for tool in ["buildifier", "gersemi", "jsonnetfmt", "stylua", "typstyle", "yamlfmt"]],
                [{}, {}, {}, {}, {}, {}],
            ),
            expect_equal(
                "the biome format and lint stages run remotely",
                [dx_stage_execution_requirements(["biome"], "format"), dx_stage_execution_requirements(["biome"], "lint")],
                [{}, {}],
            ),
            expect_equal(
                "the starlark lint stage runs remotely",
                dx_stage_execution_requirements(["buildifier"], "lint"),
                {},
            ),
        ] + [
            expect_equal(
                record["tool"] + " " + record["capability"] + " remote record validates",
                remote_record_error(record),
                "",
            )
            for record in REMOTE_QUALIFIED
        ] + [
            expect_equal(
                "pinned artifacts cover every qualified " + record["tool"] + " platform",
                remote_platform_error(record, _artifact_platforms()),
                "",
            )
            for record in REMOTE_QUALIFIED
        ] + [
            expect_match(
                "a qualified platform without a pinned artifact fails",
                remote_platform_error(SHFMT_FORMAT_REMOTE, {"shfmt": ["linux_x86_64"]}),
                "without a pinned artifact",
            ),
            expect_equal(
                "unknown tool record is rejected",
                remote_record_error(_remote_record(tool = "not_a_tool")),
                "remote: unknown tool 'not_a_tool': not in the real adapter registry",
            ),
            expect_equal(
                "unwired capability record is rejected",
                remote_record_error(_remote_record(capability = "audit")),
                "remote: tool 'shfmt' has no 'audit' capability",
            ),
            expect_equal(
                "non-declared closure record is rejected",
                remote_record_error(_remote_record(closure = "ambient")),
                "remote: tool 'shfmt' needs closure 'declared', got 'ambient'",
            ),
            expect_equal(
                "non-hermetic runner record is rejected",
                remote_record_error(_remote_record(runner = "host")),
                "remote: tool 'shfmt' needs runner 'hermetic', got 'host'",
            ),
            expect_equal(
                "unqualified state record is rejected",
                remote_record_error(_remote_record(state = "supported")),
                "remote: tool 'shfmt' needs state 'qualified', got 'supported'",
            ),
            expect_equal(
                "empty platform record is rejected",
                remote_record_error(_remote_record(platforms = [])),
                "remote: tool 'shfmt' needs a non-empty execution platform list",
            ),
            expect_equal(
                "unknown platform record is rejected",
                remote_record_error(_remote_record(platforms = ["linux_riscv64"])),
                "remote: tool 'shfmt' names unknown execution platform 'linux_riscv64'",
            ),
            expect_equal(
                "record missing a field is rejected",
                remote_record_error({"tool": "shfmt"}),
                "remote: record is missing field 'capability'",
            ),
        ],
    )
