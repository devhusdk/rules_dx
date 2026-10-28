"""Unit tests for explicit tool support records."""

load("//dart/rules:platforms.bzl", "RULES_DART_PLATFORMS")
load("//elixir/toolchain:pins.bzl", "ELIXIR_CELLS")
load("//erlang/toolchain:pins.bzl", ERLANG_CELLS = "CELLS")
load("//julia/rules:platforms.bzl", "JULIA_PLATFORMS")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load("//qt/sdk:pins.bzl", QT_CELLS = "CELLS")
load("//quality/artifacts:extension.bzl", "TOOL_ARTIFACTS")
load("//quality/artifacts:hub.bzl", "EMULATED_TOOLS", "PLATFORM_FALLBACKS")
load("//quality/testdata:runner_matrix_cases.bzl", "RUNNER_MATRIX_CASES")
load("//quality/testdata:runner_matrix_tests.bzl", "matrix_case_platforms", "matrix_platform_case_names")
load(":real_aspects.bzl", "real_wired_capabilities")
load(":support.bzl", "ANSIBLE_PRETTIER_FORMAT_SUPPORT", "API_SPEC_PRETTIER_FORMAT_SUPPORT", "API_SPEC_REDOCLY_LINT_SUPPORT", "CLOUDFORMATION_CFN_LINT_SUPPORT", "CMAKE_CMAKELINT_SUPPORT", "DOCKERFILE_PRETTIER_FORMAT_SUPPORT", "GHERKIN_GPLINT_LINT_SUPPORT", "GHERKIN_PRETTIER_FORMAT_SUPPORT", "JUPYTER_NOTEBOOK_OUTPUTS_LINT_SUPPORT", "JUPYTER_RUFF_FORMAT_SUPPORT", "JUPYTER_RUFF_LINT_SUPPORT", "KUBERNETES_PRETTIER_FORMAT_SUPPORT", "MAKE_MBAKE_SUPPORT", "PHP_PRETTIER_FORMAT_SUPPORT", "SQL_PRETTIER_FORMAT_SUPPORT", "SQL_SQLFLUFF_FORMAT_SUPPORT", "SQL_SQLFLUFF_LINT_SUPPORT", "SUPPORT_PLATFORMS", "SUPPORT_RECORDS", "TERRAFORM_FORMAT_SUPPORT", "XML_PRETTIER_FORMAT_SUPPORT", "is_support_tool", "support_gap_error", "support_record_error", "support_record_gap_error", "support_schema_error", "support_state", "support_unpinned_tools")

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

def unpinned_platforms(tool, claimed, pinned, fallbacks = PLATFORM_FALLBACKS, emulated = EMULATED_TOOLS):
    """Returns claimed platforms with no artifact: no native pin and no inventoried emulated fallback.

    A fallback only supplies an artifact for tools listed in the emulated inventory; it is never
    evidence for the claim (see unproven_claims).
    """
    missing = []
    for platform in claimed:
        if platform in pinned:
            continue
        target = fallbacks.get(platform)
        if target in pinned and tool in emulated.get(platform, []):
            continue
        missing.append(platform)
    return missing

# Claims (tool:capability:family:platform) that a support record makes today without a runner-matrix
# case that runs on that platform. The list can only shrink: an unproven claim missing from it fails,
# and an entry that becomes proven fails too. Close an entry by flagging a matrix case for the platform
# (see matrix_case_platforms), never by editing a record. Tracked by #4441 and #1383.
SUPPORT_EVIDENCE_PENDING = [
    "csc:typecheck:csharp:linux_x86_64",
    "fsc:typecheck:fsharp:linux_x86_64",
    "fsharplint:lint:fsharp:linux_x86_64",
    "gersemi:format:cmake:linux_arm64",
    "gersemi:format:cmake:macos_arm64",
    "gersemi:format:cmake:windows_x86_64",
    "helm:lint:helm:linux_arm64",
    "helm:lint:helm:linux_x86_64",
    "helm:lint:helm:macos_arm64",
    "helm:lint:helm:windows_x86_64",
    "keep_sorted:lint:text:linux_arm64",
    "keep_sorted:lint:text:linux_x86_64",
    "keep_sorted:lint:text:macos_arm64",
    "keep_sorted:lint:text:windows_x86_64",
    "kubeconform:lint:kubernetes:linux_arm64",
    "kubeconform:lint:kubernetes:linux_x86_64",
    "kubeconform:lint:kubernetes:macos_arm64",
    "kubeconform:lint:kubernetes:windows_x86_64",
    "latexindent:format:latex:linux_arm64",
    "latexindent:format:latex:macos_arm64",
    "latexindent:format:latex:windows_x86_64",
    "roslyn:lint:csharp:linux_x86_64",
    "rubocop:lint:ruby:linux_x86_64",
    "shellcheck:lint:shell:linux_arm64",
    "shellcheck:lint:shell:macos_arm64",
    "shellcheck:lint:shell:windows_x86_64",
    "shfmt:format:shell:linux_arm64",
    "shfmt:format:shell:macos_arm64",
    "shfmt:format:shell:windows_x86_64",
]

def _case_proves(case, record, platform):
    """Reports whether one matrix case is evidence for one (record, platform) claim."""
    if case.get("capability") != record["capability"] or record["tool"] not in case.get("tool_names", []):
        return False
    if platform not in matrix_case_platforms(case):
        return False
    for stage in case["stages"]:
        fields = stage.split(";")
        if len(fields) > 1 and fields[1] in record["classes"]:
            return True
    return False

def unproven_claims(records, cases):
    """Returns the sorted claim keys of records no matrix case proves on the claimed platform."""
    unproven = []
    for record in records:
        for platform in record["platforms"]:
            proven = False
            for case in cases:
                if _case_proves(case, record, platform):
                    proven = True
                    break
            if not proven:
                unproven.append(":".join([record["tool"], record["capability"], record["family"], platform]))
    return sorted(unproven)

def _evidence_checks():
    """Checks the evidence gate with synthetic cases and the real data."""
    record = {"capability": "format", "classes": ["yaml"], "family": "yaml", "platforms": ["linux_x86_64", "macos_arm64"], "tool": "yamlfmt"}
    case = {"capability": "format", "stages": ["yamlfmt;yaml;matrix/a.yaml"], "tool_names": ["yamlfmt"]}
    return [
        expect_equal(
            "emulated tool claiming windows_arm64 passes the pin check",
            unpinned_platforms("tflint", ["windows_x86_64", "windows_arm64"], ["windows_x86_64"], {"windows_arm64": "windows_x86_64"}, {"windows_arm64": ["tflint"]}),
            [],
        ),
        expect_equal(
            "tool outside the emulated inventory fails the pin check",
            unpinned_platforms("regal", ["windows_arm64"], ["windows_x86_64"], {"windows_arm64": "windows_x86_64"}, {"windows_arm64": ["tflint"]}),
            ["windows_arm64"],
        ),
        expect_equal(
            "emulated tool without a fallback pin fails the pin check",
            unpinned_platforms("tflint", ["windows_arm64"], ["linux_x86_64"], {"windows_arm64": "windows_x86_64"}, {"windows_arm64": ["tflint"]}),
            ["windows_arm64"],
        ),
        expect_equal(
            "a windows_x86_64 flag does not prove a windows_arm64 claim",
            unproven_claims([dict(record, platforms = ["windows_arm64"])], [dict(case, windows_x86_64 = True)]),
            ["yamlfmt:format:yaml:windows_arm64"],
        ),
        expect_equal(
            "a windows_arm64 flag proves a windows_arm64 claim",
            unproven_claims([dict(record, platforms = ["windows_arm64"])], [dict(case, windows_x86_64 = True, windows_arm64 = True)]),
            [],
        ),
        expect_equal(
            "matrix_case_platforms without flags is linux_x86_64",
            matrix_case_platforms({}),
            ["linux_x86_64"],
        ),
        expect_equal(
            "matrix_case_platforms linux_arm64 only",
            matrix_case_platforms({"linux_arm64": True}),
            ["linux_x86_64", "linux_arm64"],
        ),
        expect_equal(
            "matrix_case_platforms mac and windows x64",
            matrix_case_platforms({"macos_arm64": True, "windows_x86_64": True}),
            ["linux_x86_64", "macos_arm64", "windows_x86_64"],
        ),
        expect_equal(
            "matrix_case_platforms windows x64 and arm64",
            matrix_case_platforms({"windows_x86_64": True, "windows_arm64": True}),
            ["linux_x86_64", "windows_x86_64", "windows_arm64"],
        ),
        expect_equal(
            "an unflagged case is in no non-default platform suite",
            [matrix_platform_case_names([{"name": "a"}], p) for p in ["linux_arm64", "macos_arm64", "windows_x86_64", "windows_arm64"]],
            [[], [], [], []],
        ),
        expect_equal(
            "a windows_arm64-only case is in that suite only",
            [matrix_platform_case_names([{"name": "a", "windows_arm64": True}, {"name": "b"}], p) for p in ["linux_arm64", "macos_arm64", "windows_x86_64", "windows_arm64"]],
            [[], [], [], ["a"]],
        ),
        expect_equal(
            "suite membership matches matrix_case_platforms",
            [matrix_platform_case_names([{"name": "a", "macos_arm64": True, "linux_arm64": True}], p) for p in ["linux_arm64", "macos_arm64", "windows_x86_64"]],
            [["a"], ["a"], []],
        ),
        expect_equal(
            "a flagged matching case proves the claim and the linux baseline",
            unproven_claims([record], [dict(case, macos_arm64 = True)]),
            [],
        ),
        expect_equal(
            "an unflagged case proves only linux_x86_64",
            unproven_claims([record], [case]),
            ["yamlfmt:format:yaml:macos_arm64"],
        ),
        expect_equal(
            "a case with another capability is not evidence",
            unproven_claims([record], [dict(case, capability = "lint", macos_arm64 = True)]),
            ["yamlfmt:format:yaml:linux_x86_64", "yamlfmt:format:yaml:macos_arm64"],
        ),
        expect_equal(
            "a case with another class is not evidence",
            unproven_claims([record], [dict(case, stages = ["yamlfmt;json;matrix/a.json"], macos_arm64 = True)]),
            ["yamlfmt:format:yaml:linux_x86_64", "yamlfmt:format:yaml:macos_arm64"],
        ),
        expect_equal(
            "a case for another tool is not evidence",
            unproven_claims([record], [dict(case, tool_names = ["yamllint"], macos_arm64 = True)]),
            ["yamlfmt:format:yaml:linux_x86_64", "yamlfmt:format:yaml:macos_arm64"],
        ),
        expect_equal(
            "pending evidence list is sorted",
            SUPPORT_EVIDENCE_PENDING,
            sorted(SUPPORT_EVIDENCE_PENDING),
        ),
        expect_equal(
            "unproven claims equal the pending list exactly",
            unproven_claims(SUPPORT_RECORDS, RUNNER_MATRIX_CASES),
            SUPPORT_EVIDENCE_PENDING,
        ),
        expect_equal(
            "windows_arm64 is a valid unsupported platform",
            support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_x86_64"], "state": "known", "tool": "keep_sorted", "transport": "single", "unsupported": ["windows_arm64"]}),
            "",
        ),
        expect_equal(
            "unknown platform in platforms is rejected",
            support_record_error({"artifact": "missing", "capability": "lint", "classes": ["text"], "context": "owned", "execution": "direct", "family": "text", "fix": "supported", "platforms": ["linux_amd64"], "state": "known", "tool": "keep_sorted", "transport": "single"}),
            "support: unknown platform 'linux_amd64' for tool 'keep_sorted'",
        ),
    ]

def _canonical(cells):
    """Maps toolchain cell names to support platform names (amd64 is x86_64)."""
    return [cell.replace("amd64", "x86_64") for cell in cells]

# Platforms of the language toolchain that supplies each tool. A record may only claim a platform
# inside its toolchain's cells; widening a cell and flagging a matrix case widens the claim.
def _toolchain_platforms():
    return {
        "credo": _canonical(ELIXIR_CELLS),
        "dart": sorted(RULES_DART_PLATFORMS.keys()),
        "elvis": _canonical(ERLANG_CELLS.keys()),
        "erlfmt": _canonical(ERLANG_CELLS.keys()),
        "julia_format": sorted(JULIA_PLATFORMS.keys()),
        "mix_format": _canonical(ELIXIR_CELLS),
        "qmlformat": [name for name, cell in QT_CELLS.items() if not getattr(cell, "blocker", "")],
        "qmllint": [name for name, cell in QT_CELLS.items() if not getattr(cell, "blocker", "")],
    }

# Records that must name every platform as either supported or unsupported with a reason comment.
_COMPLETE_TOOLS = ["credo", "dart", "elvis", "erlfmt", "hlint", "julia_format", "mix_format", "ormolu", "qmlformat", "qmllint", "zig_fmt"]

def _toolchain_checks():
    """Checks toolchain-supplied claims stay inside the toolchain and cover all platforms."""
    checks = []
    cells = _toolchain_platforms()
    for record in SUPPORT_RECORDS:
        tool = record["tool"]
        label = tool + " " + record["capability"]
        if tool in cells:
            checks.append(expect_equal(
                label + " claims stay inside its toolchain platforms",
                [p for p in record["platforms"] if p not in cells[tool]],
                [],
            ))
        if tool in _COMPLETE_TOOLS:
            checks.append(expect_equal(
                label + " platforms plus unsupported cover every platform",
                sorted(record["platforms"] + record.get("unsupported", [])),
                sorted(SUPPORT_PLATFORMS),
            ))
    return checks

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
                unpinned_platforms(tool, record["platforms"], artifact_platforms[tool]),
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
                ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            expect_equal(
                "jupyter ruff records cover five platforms",
                [JUPYTER_RUFF_FORMAT_SUPPORT["platforms"], JUPYTER_RUFF_LINT_SUPPORT["platforms"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"], ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]],
            ),
            expect_equal(
                "jupyter notebook_outputs lint record covers four platforms",
                JUPYTER_NOTEBOOK_OUTPUTS_LINT_SUPPORT["platforms"],
                ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            expect_equal(
                "sql xml and gherkin prettier records cover four platforms",
                [SQL_PRETTIER_FORMAT_SUPPORT["platforms"], XML_PRETTIER_FORMAT_SUPPORT["platforms"], GHERKIN_PRETTIER_FORMAT_SUPPORT["platforms"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"], ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"], ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]],
            ),
            expect_equal(
                "gherkin gplint lint record is owned, batched, check-only and linux_x86_64 only",
                [GHERKIN_GPLINT_LINT_SUPPORT["platforms"], GHERKIN_GPLINT_LINT_SUPPORT["fix"], GHERKIN_GPLINT_LINT_SUPPORT["transport"], GHERKIN_GPLINT_LINT_SUPPORT["context"]],
                [["linux_x86_64"], "check_only", "batched", "owned"],
            ),
            expect_equal(
                "api_spec prettier format record covers four platforms",
                API_SPEC_PRETTIER_FORMAT_SUPPORT["platforms"],
                ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            expect_equal(
                "ansible and kubernetes prettier records cover four platforms",
                [ANSIBLE_PRETTIER_FORMAT_SUPPORT["platforms"], KUBERNETES_PRETTIER_FORMAT_SUPPORT["platforms"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]] * 2,
            ),
            expect_equal(
                "cloudformation cfn_lint lint record is owned, batched, check-only and covers four platforms",
                [CLOUDFORMATION_CFN_LINT_SUPPORT["platforms"], CLOUDFORMATION_CFN_LINT_SUPPORT["fix"], CLOUDFORMATION_CFN_LINT_SUPPORT["transport"], CLOUDFORMATION_CFN_LINT_SUPPORT["context"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"], "check_only", "batched", "owned"],
            ),
            expect_equal(
                "cmake cmakelint lint record is owned, batched, check-only and covers four platforms",
                [CMAKE_CMAKELINT_SUPPORT["platforms"], CMAKE_CMAKELINT_SUPPORT["fix"], CMAKE_CMAKELINT_SUPPORT["transport"], CMAKE_CMAKELINT_SUPPORT["context"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"], "check_only", "batched", "owned"],
            ),
            expect_equal(
                "make mbake format record is owned, batched, fixable and proven on linux_x86_64",
                [MAKE_MBAKE_SUPPORT["platforms"], MAKE_MBAKE_SUPPORT["fix"], MAKE_MBAKE_SUPPORT["transport"], MAKE_MBAKE_SUPPORT["context"]],
                [["linux_x86_64"], "supported", "batched", "owned"],
            ),
            expect_equal(
                "sql sqlfluff format record is owned, batched, fixable and covers four platforms",
                [SQL_SQLFLUFF_FORMAT_SUPPORT["platforms"], SQL_SQLFLUFF_FORMAT_SUPPORT["fix"], SQL_SQLFLUFF_FORMAT_SUPPORT["transport"], SQL_SQLFLUFF_FORMAT_SUPPORT["context"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"], "supported", "batched", "owned"],
            ),
            expect_equal(
                "sql sqlfluff lint record is owned, batched, check-only and covers four platforms",
                [SQL_SQLFLUFF_LINT_SUPPORT["platforms"], SQL_SQLFLUFF_LINT_SUPPORT["fix"], SQL_SQLFLUFF_LINT_SUPPORT["transport"], SQL_SQLFLUFF_LINT_SUPPORT["context"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"], "check_only", "batched", "owned"],
            ),
            expect_equal(
                "dockerfile and php prettier records cover four platforms",
                [DOCKERFILE_PRETTIER_FORMAT_SUPPORT["platforms"], PHP_PRETTIER_FORMAT_SUPPORT["platforms"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]] * 2,
            ),
            expect_equal(
                "api_spec redocly lint record covers four platforms",
                API_SPEC_REDOCLY_LINT_SUPPORT["platforms"],
                ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
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
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "known", "tool": "chktex", "transport": "single", "unsupported": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]}),
                "",
            ),
            expect_equal(
                "empty platforms need every platform unsupported",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "known", "tool": "chktex", "transport": "single", "unsupported": ["linux_x86_64"]}),
                "support: tool 'chktex' needs a non-empty platform list",
            ),
            expect_equal(
                "empty platforms are rejected for a supported record",
                support_record_error({"artifact": "pinned", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "supported", "tool": "chktex", "transport": "single", "unsupported": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]}),
                "support: tool 'chktex' needs a non-empty platform list",
            ),
            expect_equal(
                "empty platforms are rejected for a qualified record",
                support_record_error({"artifact": "missing", "capability": "lint", "classes": ["latex"], "context": "owned", "execution": "direct", "family": "latex", "fix": "check_only", "platforms": [], "state": "qualified", "tool": "chktex", "transport": "single", "unsupported": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]}),
                "support: tool 'chktex' needs a non-empty platform list",
            ),
            expect_equal(
                "only tools with no build anywhere are unpinned",
                [support_unpinned_tools("lint"), support_unpinned_tools("format")],
                [{"chktex": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"]}, {}],
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
        ] + _evidence_checks() + _toolchain_checks() + _record_checks(real_wired_capabilities(), _artifact_platforms()),
    )
