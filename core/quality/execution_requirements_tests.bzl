"""Unit tests for the execution-requirements helper."""

load("@dx_pins_data//:pins.bzl", "DATA")
load("//core/private/testing:defs.bzl", "expect_equal", "expect_false", "expect_match", "expect_true", "starlark_test")
load("//core/quality:adapters.bzl", "REAL_ADAPTERS")
load(":execution_requirements.bzl", "DX_FORBIDDEN_NO_REMOTE", "DX_NO_REMOTE_EXEC", "LOCAL_ONLY", "PENDING", "REMOTE_QUALIFIED", "SHFMT_FORMAT_REMOTE", "classification_errors", "dx_has_forbidden_marker", "dx_is_local_only", "dx_is_remote_qualified", "dx_remote_platforms", "dx_stage_execution_requirements", "remote_platform_error", "remote_record_error")

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
                "prettier format stays unqualified",
                dx_is_remote_qualified("prettier", "format"),
            ),
            expect_equal(
                "shfmt-only format stages lose the local-only marker",
                dx_stage_execution_requirements(["shfmt"], "format"),
                {},
            ),
            expect_equal(
                "unqualified tool stages stay local-only",
                dx_stage_execution_requirements(["prettier"], "format"),
                {"no-remote-exec": "1"},
            ),
            expect_equal(
                "mixed qualified and unqualified stages stay local-only",
                dx_stage_execution_requirements(["shfmt", "prettier"], "format"),
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
                "taplo is qualified for format and lint, not typecheck",
                [dx_is_remote_qualified("taplo", "format"), dx_is_remote_qualified("taplo", "lint"), dx_is_remote_qualified("taplo", "typecheck")],
                [True, True, False],
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
            expect_equal(
                "the workflow lint stage runs remotely",
                dx_stage_execution_requirements(["actionlint", "zizmor"], "lint"),
                {},
            ),
            expect_equal(
                "the vale lint stage runs remotely without converters",
                dx_stage_execution_requirements(["vale"], "lint"),
                {},
            ),
            expect_equal(
                "a vale lint stage with rst2html runs remotely",
                dx_stage_execution_requirements(["vale"], "lint", run_tools = ["rst2html"]),
                {},
            ),
            expect_equal(
                "a vale lint stage with asciidoctor runs remotely",
                dx_stage_execution_requirements(["vale"], "lint", run_tools = ["asciidoctor"]),
                {},
            ),
            expect_equal(
                "a vale lint stage with both converters runs remotely",
                dx_stage_execution_requirements(["vale"], "lint", run_tools = ["asciidoctor", "rst2html"]),
                {},
            ),
            expect_equal(
                "a qualified launcher beside an unqualified one stays local",
                dx_stage_execution_requirements(["vale"], "lint", run_tools = ["asciidoctor", "x"]),
                {DX_NO_REMOTE_EXEC: "1"},
            ),
            expect_equal(
                "a qualified launcher in an unqualified stage stays local",
                dx_stage_execution_requirements(["eslint"], "lint", run_tools = ["rst2html"]),
                {DX_NO_REMOTE_EXEC: "1"},
            ),
            expect_equal(
                "any stage with a runfiles launcher stays local",
                dx_stage_execution_requirements(["shfmt"], "format", run_tools = ["x"]),
                {DX_NO_REMOTE_EXEC: "1"},
            ),
            expect_equal(
                "the ruff lint stage runs remotely",
                dx_stage_execution_requirements(["ruff"], "lint"),
                {},
            ),
            expect_equal(
                "the ruff format stage runs remotely",
                dx_stage_execution_requirements(["ruff"], "format"),
                {},
            ),
            expect_equal(
                "ruff lint beside pydoclint stays local",
                dx_stage_execution_requirements(["ruff", "pydoclint"], "lint"),
                {DX_NO_REMOTE_EXEC: "1"},
            ),
            expect_equal(
                "the taplo lint, hadolint and checkmake stages run remotely",
                [
                    dx_stage_execution_requirements(["taplo"], "lint"),
                    dx_stage_execution_requirements(["hadolint"], "lint"),
                    dx_stage_execution_requirements(["checkmake"], "lint"),
                ],
                [{}, {}, {}],
            ),
            expect_equal(
                "the cue, ffmt, fortitude, jsonnet_lint, opa and xmllint stages run remotely",
                [
                    dx_stage_execution_requirements(["cue"], "format"),
                    dx_stage_execution_requirements(["cue"], "typecheck"),
                    dx_stage_execution_requirements(["ffmt"], "format"),
                    dx_stage_execution_requirements(["fortitude"], "lint"),
                    dx_stage_execution_requirements(["jsonnet_lint"], "lint"),
                    dx_stage_execution_requirements(["opa"], "format"),
                    dx_stage_execution_requirements(["opa"], "typecheck"),
                    dx_stage_execution_requirements(["xmllint"], "lint"),
                ],
                [{}, {}, {}, {}, {}, {}, {}, {}],
            ),
            expect_equal(
                "regal lint runs remotely",
                dx_stage_execution_requirements(["regal"], "lint"),
                {},
            ),
            expect_equal(
                "the kubeconform, tflint, helmfmt and trivy_image_config stages run remotely",
                [
                    dx_stage_execution_requirements(["kubeconform"], "lint"),
                    dx_stage_execution_requirements(["tflint"], "lint"),
                    dx_stage_execution_requirements(["helmfmt"], "format"),
                    dx_stage_execution_requirements(["trivy_image_config"], "lint"),
                ],
                [{}, {}, {}, {}],
            ),
            expect_equal(
                "kubeconform and tflint beside trivy_config stay local",
                [
                    dx_stage_execution_requirements(["kubeconform", "trivy_config"], "lint"),
                    dx_stage_execution_requirements(["tflint", "trivy_config"], "lint"),
                ],
                [{DX_NO_REMOTE_EXEC: "1"}, {DX_NO_REMOTE_EXEC: "1"}],
            ),
            expect_match(
                "an adapter without its own pinned artifact fails without the artifact key",
                remote_platform_error(_remote_record(tool = "trivy_image_config", capability = "lint"), _artifact_platforms()),
                "without a pinned artifact",
            ),
            expect_equal(
                "a qualified tool beside an unqualified one stays local",
                dx_stage_execution_requirements(["actionlint", "lychee"], "lint"),
                {DX_NO_REMOTE_EXEC: "1"},
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
                "every real tool and capability is in exactly one of the qualified, local-only and pending lists",
                classification_errors(REAL_ADAPTERS, REMOTE_QUALIFIED, LOCAL_ONLY, PENDING),
                [],
            ),
            expect_equal(
                "lychee lint stays local",
                [dx_stage_execution_requirements(["lychee"], "lint"), LOCAL_ONLY[("lychee", "lint")] != ""],
                [{DX_NO_REMOTE_EXEC: "1"}, True],
            ),
            expect_equal(
                "an unlisted pair is reported",
                classification_errors({"a": {"lint": []}}, [], {}, []),
                ["remote: 'a lint' is in none of REMOTE_QUALIFIED, LOCAL_ONLY or PENDING"],
            ),
            expect_equal(
                "a pair in two lists is reported",
                classification_errors({"shfmt": {"format": []}}, [SHFMT_FORMAT_REMOTE], {}, [("shfmt", "format")]),
                ["remote: 'shfmt format' is in both REMOTE_QUALIFIED and PENDING"],
            ),
            expect_equal(
                "an empty local-only reason is reported",
                classification_errors({"a": {"lint": []}}, [], {("a", "lint"): " "}, []),
                ["remote: LOCAL_ONLY 'a lint' needs a reason"],
            ),
            expect_equal(
                "a listed pair that is not a real adapter is reported",
                classification_errors({}, [], {}, [("a", "lint")]),
                ["remote: PENDING lists 'a lint', which is not a real adapter"],
            ),
            expect_equal(
                "record missing a field is rejected",
                remote_record_error({"tool": "shfmt"}),
                "remote: record is missing field 'capability'",
            ),
        ],
    )
