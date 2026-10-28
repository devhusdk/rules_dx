"""Unit tests for explicit tool support records."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//quality/artifacts:extension.bzl", "TOOL_ARTIFACTS")
load(":real_aspects.bzl", "real_wired_capabilities")
load(":support.bzl", "API_SPEC_REDOCLY_LINT_SUPPORT", "GHERKIN_PRETTIER_FORMAT_SUPPORT", "JUPYTER_NOTEBOOK_OUTPUTS_LINT_SUPPORT", "JUPYTER_RUFF_FORMAT_SUPPORT", "JUPYTER_RUFF_LINT_SUPPORT", "SQL_PRETTIER_FORMAT_SUPPORT", "SUPPORT_RECORDS", "TERRAFORM_FORMAT_SUPPORT", "XML_PRETTIER_FORMAT_SUPPORT", "is_support_tool", "support_gap_error", "support_record_error", "support_record_gap_error", "support_schema_error", "support_state", "support_unpinned_tools")

def _without(wired, tool, capability):
    """Returns a copy of the wired map with one tool capability removed."""
    copy = {name: list(caps) for name, caps in wired.items()}
    copy[tool] = [cap for cap in copy.get(tool, []) if cap != capability]
    return copy

def _artifact_platforms():
    """Returns the pinned artifact platforms keyed by tool."""
    by_tool = {}
    for artifact in TOOL_ARTIFACTS:
        by_tool.setdefault(artifact["tool"], []).append(artifact["os"] + "_" + artifact["cpu"])
    return by_tool

def _record_checks(wired, artifact_platforms):
    """Builds the per-record checks over every support record."""
    checks = []
    for record in SUPPORT_RECORDS:
        tool = record["tool"]
        capability = record["capability"]
        label = record["family"] + " " + tool + " " + capability
        checks.append(expect_equal(
            label + " record is valid",
            support_record_error(record),
            "",
        ))
        if record["state"] == "known":
            checks.append(expect_equal(
                label + " is a known, ungated support tool",
                [is_support_tool(tool), support_record_gap_error(record, {})],
                [True, ""],
            ))
            continue
        checks.append(expect_equal(
            label + " is a support tool in a gated state",
            [is_support_tool(tool), record["state"] in ["qualified", "supported"]],
            [True, True],
        ))
        checks.append(expect_equal(
            label + " is checked for wiring",
            support_record_gap_error(record, _without(wired, tool, capability)),
            "support: " + tool + " " + capability + " is " + record["state"] + " but has no wired aspect (not silently clean)",
        ))
        if tool in artifact_platforms:
            checks.append(expect_equal(
                label + " platforms are pinned",
                [platform for platform in record["platforms"] if platform not in artifact_platforms[tool]],
                [],
            ))
    return checks

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
                "every supported record is wired and pinned",
                support_gap_error(real_wired_capabilities()),
                "",
            ),
            expect_equal(
                "unknown tools stay outside support",
                [is_support_tool("ty"), support_state("ty")],
                [False, "unknown"],
            ),
            expect_equal(
                "terraform format record covers four platforms",
                TERRAFORM_FORMAT_SUPPORT["platforms"],
                ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
            ),
            expect_equal(
                "jupyter ruff records cover four platforms",
                [JUPYTER_RUFF_FORMAT_SUPPORT["platforms"], JUPYTER_RUFF_LINT_SUPPORT["platforms"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]] * 2,
            ),
            expect_equal(
                "jupyter notebook_outputs lint record covers four platforms",
                JUPYTER_NOTEBOOK_OUTPUTS_LINT_SUPPORT["platforms"],
                ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
            ),
            expect_equal(
                "sql xml and gherkin prettier records cover four platforms",
                [SQL_PRETTIER_FORMAT_SUPPORT["platforms"], XML_PRETTIER_FORMAT_SUPPORT["platforms"], GHERKIN_PRETTIER_FORMAT_SUPPORT["platforms"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]] * 3,
            ),
            expect_equal(
                "api_spec redocly lint record covers four platforms",
                API_SPEC_REDOCLY_LINT_SUPPORT["platforms"],
                ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
            ),
            expect_equal(
                "supported record without a pinned artifact is an actionable failure",
                support_record_gap_error({"artifact": "missing", "capability": "lint", "state": "supported", "tool": "keep_sorted"}, {"keep_sorted": ["lint"]}),
                "support: keep_sorted lint is supported but has no pinned artifact",
            ),
            expect_equal(
                "known record is not gated",
                support_record_gap_error({"artifact": "missing", "capability": "lint", "state": "known", "tool": "keep_sorted"}, {}),
                "",
            ),
            expect_equal(
                "qualified record without wiring is an actionable failure",
                support_record_gap_error({"artifact": "pinned", "capability": "lint", "state": "qualified", "tool": "keep_sorted"}, {}),
                "support: keep_sorted lint is qualified but has no wired aspect (not silently clean)",
            ),

            expect_equal(
                "an unbuilt known record may have no platforms",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "known", "tool": "chktex", "transport": "single", "unsupported": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]}),
                "",
            ),
            expect_equal(
                "empty platforms need every platform unsupported",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "known", "tool": "chktex", "transport": "single", "unsupported": ["linux_x86_64"]}),
                "support: tool 'chktex' needs a non-empty platform list",
            ),
            expect_equal(
                "empty platforms are rejected for a supported record",
                support_record_error({"artifact": "pinned", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "supported", "tool": "chktex", "transport": "single", "unsupported": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]}),
                "support: tool 'chktex' needs a non-empty platform list",
            ),
            expect_equal(
                "empty platforms are rejected for a qualified record",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "qualified", "tool": "chktex", "transport": "single", "unsupported": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]}),
                "support: tool 'chktex' needs a non-empty platform list",
            ),
            expect_equal(
                "only tools with no build anywhere are unpinned",
                [support_unpinned_tools("lint"), support_unpinned_tools("format")],
                [{"chktex": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]}, {}],
            ),
            expect_equal(
                "valid unsupported platform is accepted",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single", "unsupported": ["windows_x86_64"]}),
                "",
            ),
            expect_equal(
                "unsupported overlapping platforms is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single", "unsupported": ["linux_x86_64"]}),
                "support: platform 'linux_x86_64' is both supported and unsupported for tool 'keep_sorted'",
            ),
            expect_equal(
                "unknown unsupported platform is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single", "unsupported": ["plan9_x86_64"]}),
                "support: unknown unsupported platform 'plan9_x86_64' for tool 'keep_sorted'",
            ),
            expect_equal(
                "non-list unsupported is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single", "unsupported": "windows_x86_64"}),
                "support: tool 'keep_sorted' needs 'unsupported' to be a list",
            ),
            expect_equal(
                "unknown tool is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "not_a_tool", "transport": "single"}),
                "support: unknown tool 'not_a_tool': not in the real adapter registry",
            ),
            expect_equal(
                "unknown capability is rejected",
                support_record_error({"artifact": "missing", "capability": "audit", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single"}),
                "support: tool 'keep_sorted' has no 'audit' capability",
            ),
            expect_equal(
                "unclassified class is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["not_a_class"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single"}),
                "support: tool 'keep_sorted' names unclassified class 'not_a_class'",
            ),
            expect_equal(
                "wrong family is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "python", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single"}),
                "support: class 'text' belongs to family 'text', not 'python'",
            ),
            expect_equal(
                "class outside adapter support is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["python"], "context": "owned", "execution": "direct", "family": "python", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single"}),
                "support: class 'python' is outside 'keep_sorted lint' adapter support",
            ),
            expect_equal(
                "bad execution is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "fork", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single"}),
                "support: tool 'keep_sorted' needs execution 'direct' or 'delegated', got 'fork'",
            ),
            expect_equal(
                "bad fix is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "rewrite", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single"}),
                "support: tool 'keep_sorted' needs fix 'supported' or 'check_only', got 'rewrite'",
            ),
            expect_equal(
                "bad state is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "maybe", "tool": "keep_sorted", "transport": "single"}),
                "support: tool 'keep_sorted' needs state 'known', 'supported' or 'qualified', got 'maybe'",
            ),
            expect_equal(
                "missing transport is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted"}),
                "support: record is missing field 'transport'",
            ),
            expect_equal(
                "bad transport is rejected",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "parcel"}),
                "support: tool 'keep_sorted' needs transport 'batched' or 'single', got 'parcel'",
            ),
        ] + _record_checks(real_wired_capabilities(), _artifact_platforms()),
    )
