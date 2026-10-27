"""Unit tests for explicit tool support records."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":adapters.bzl", "real_supported_classes")
load(":real_aspects.bzl", "real_wired_tools")
load(":support.bzl", "API_SPEC_REDOCLY_LINT_SUPPORT", "CSHARP_CSHARPIER_SUPPORT", "CSHARP_ROSLYN_SUPPORT", "CSS_STYLELINT_LINT_SUPPORT", "CUE_FORMAT_SUPPORT", "DOCKERFILE_HADOLINT_SUPPORT", "FSHARP_FANTOMAS_SUPPORT", "FSHARP_FSHARPLINT_SUPPORT", "GHERKIN_PRETTIER_FORMAT_SUPPORT", "GO_GOFUMPT_FORMAT_SUPPORT", "GRAPHQL_ESLINT_LINT_SUPPORT", "HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT", "HTML_TEMPLATE_DJLINT_LINT_SUPPORT", "JSONNET_FORMAT_SUPPORT", "MDX_REMARK_FORMAT_SUPPORT", "PKL_FORMAT_SUPPORT", "POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT", "PROTOBUF_BUF_FORMAT_SUPPORT", "PROTOBUF_BUF_LINT_SUPPORT", "RUBY_RUBOCOP_SUPPORT", "RUBY_STANDARDRB_SUPPORT", "SCALA_SCALAFIX_SUPPORT", "SCALA_SCALAFMT_SUPPORT", "SHELL_SHELLCHECK_SUPPORT", "SHELL_SHFMT_SUPPORT", "SQL_PRETTIER_FORMAT_SUPPORT", "TERRAFORM_FORMAT_SUPPORT", "TEXT_KEEP_SORTED_SUPPORT", "XML_PRETTIER_FORMAT_SUPPORT", "YAML_YAMLFMT_SUPPORT", "YAML_YAMLLINT_SUPPORT", "is_support_tool", "support_api_spec_artifact_error", "support_api_spec_cli_error", "support_api_spec_wiring_error", "support_artifact_error", "support_cli_error", "support_csharp_artifact_error", "support_csharp_cli_error", "support_csharp_wiring_error", "support_css_artifact_error", "support_css_cli_error", "support_css_wiring_error", "support_cue_artifact_error", "support_cue_cli_error", "support_cue_wiring_error", "support_dockerfile_artifact_error", "support_dockerfile_cli_error", "support_dockerfile_wiring_error", "support_fsharp_artifact_error", "support_fsharp_wiring_error", "support_go_artifact_error", "support_go_cli_error", "support_go_wiring_error", "support_graphql_artifact_error", "support_graphql_cli_error", "support_graphql_wiring_error", "support_html_template_artifact_error", "support_html_template_cli_error", "support_html_template_wiring_error", "support_jsonnet_artifact_error", "support_jsonnet_cli_error", "support_jsonnet_wiring_error", "support_mdx_artifact_error", "support_mdx_cli_error", "support_mdx_wiring_error", "support_pkl_artifact_error", "support_pkl_cli_error", "support_pkl_wiring_error", "support_powershell_artifact_error", "support_powershell_cli_error", "support_powershell_wiring_error", "support_prettier_plugins_artifact_error", "support_prettier_plugins_cli_error", "support_prettier_plugins_wiring_error", "support_protobuf_artifact_error", "support_protobuf_cli_error", "support_protobuf_wiring_error", "support_record_error", "support_ruby_artifact_error", "support_ruby_wiring_error", "support_scala_artifact_error", "support_scala_cli_error", "support_scala_wiring_error", "support_schema_error", "support_shell_artifact_error", "support_shell_cli_error", "support_shell_wiring_error", "support_state", "support_terraform_artifact_error", "support_terraform_cli_error", "support_terraform_wiring_error", "support_wiring_error", "support_yaml_artifact_error", "support_yaml_cli_error", "support_yaml_wiring_error")

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
                "keep_sorted is a support tool with qualified state",
                [is_support_tool("keep_sorted"), support_state("keep_sorted")],
                [True, "qualified"],
            ),
            expect_equal(
                "unknown tools stay outside support",
                [is_support_tool("ruff"), support_state("ruff")],
                [False, "unknown"],
            ),
            expect_equal(
                "missing aspect wiring is an actionable failure",
                support_wiring_error([]),
                "support: keep_sorted lint is qualified but has no wired aspect (missing real_text_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "wired keep_sorted clears the aspect gap",
                support_wiring_error(["keep_sorted"]),
                "",
            ),
            expect_equal(
                "missing CLI aspect is an actionable failure",
                support_cli_error(["@rules_dx//quality:real_aspects.bzl%real_lint_aspect"]),
                "support: keep_sorted lint is qualified but has no CLI lint aspect (missing real_text_lint_aspect)",
            ),
            expect_equal(
                "CLI text aspect clears the CLI gap",
                support_cli_error(["@rules_dx//quality:real_aspects.bzl%real_text_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned artifact clears the artifact gap",
                support_artifact_error(),
                "",
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
            expect_equal(
                "record keeps the migrated platforms and fix",
                [TEXT_KEEP_SORTED_SUPPORT["platforms"], TEXT_KEEP_SORTED_SUPPORT["fix"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"], "supported"],
            ),
            expect_equal(
                "record keeps the batched transport",
                TEXT_KEEP_SORTED_SUPPORT["transport"],
                "batched",
            ),
            expect_equal(
                "shell records validate",
                [support_record_error(SHELL_SHELLCHECK_SUPPORT), support_record_error(SHELL_SHFMT_SUPPORT)],
                ["", ""],
            ),
            expect_equal(
                "shell tools are support tools with qualified state",
                [is_support_tool("shellcheck"), support_state("shellcheck"), is_support_tool("shfmt"), support_state("shfmt")],
                [True, "qualified", True, "qualified"],
            ),
            expect_equal(
                "shell records keep the direct single execution",
                [[SHELL_SHELLCHECK_SUPPORT["execution"], SHELL_SHELLCHECK_SUPPORT["transport"]], [SHELL_SHFMT_SUPPORT["execution"], SHELL_SHFMT_SUPPORT["transport"]]],
                [["direct", "single"], ["direct", "single"]],
            ),
            expect_equal(
                "shell records keep the check and fix support",
                [SHELL_SHELLCHECK_SUPPORT["fix"], SHELL_SHFMT_SUPPORT["fix"]],
                ["check_only", "supported"],
            ),
            expect_equal(
                "shell records keep the migrated platforms",
                [SHELL_SHELLCHECK_SUPPORT["platforms"], SHELL_SHFMT_SUPPORT["platforms"]],
                [["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"], ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]],
            ),
            expect_equal(
                "missing shell aspect wiring is an actionable failure",
                support_shell_wiring_error([]),
                "support: shellcheck lint is qualified but has no wired aspect (missing real_shell_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "half-wired shell still names the missing aspect",
                support_shell_wiring_error(["shellcheck"]),
                "support: shfmt format is qualified but has no wired aspect (missing real_shell_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired shell clears the aspect gap",
                support_shell_wiring_error(["shellcheck", "shfmt"]),
                "",
            ),
            expect_equal(
                "missing shell CLI aspect is an actionable failure",
                support_shell_cli_error(["@rules_dx//quality:real_aspects.bzl%real_lint_aspect"]),
                "support: shellcheck lint is qualified but has no CLI lint aspect (missing real_shell_lint_aspect)",
            ),
            expect_equal(
                "lint-only shell CLI still names the format gap",
                support_shell_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_lint_aspect"]),
                "support: shfmt format is qualified but has no CLI format aspect (missing real_shell_format_aspect)",
            ),
            expect_equal(
                "both shell CLI aspects clear the CLI gap",
                support_shell_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_lint_aspect", "@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "",
            ),
            expect_equal(
                "scalafmt is a support tool with supported state",
                [is_support_tool("scalafmt"), support_state("scalafmt")],
                [True, "supported"],
            ),
            expect_equal(
                "scalafmt record keeps the direct batched format with fix support",
                [SCALA_SCALAFMT_SUPPORT["execution"], SCALA_SCALAFMT_SUPPORT["transport"], SCALA_SCALAFMT_SUPPORT["capability"], SCALA_SCALAFMT_SUPPORT["fix"]],
                ["direct", "batched", "format", "supported"],
            ),
            expect_equal(
                "missing scala aspect wiring is an actionable failure",
                support_scala_wiring_error([]),
                "support: scalafmt format is qualified but has no wired aspect (missing real_scala_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired scala clears the aspect gap",
                support_scala_wiring_error(["scalafmt", "scalafix"]),
                "",
            ),
            expect_equal(
                "missing scala CLI aspect is an actionable failure",
                support_scala_cli_error(["@rules_dx//quality:real_aspects.bzl%real_lint_aspect"]),
                "support: scalafmt format is qualified but has no CLI format aspect (missing real_scala_format_aspect)",
            ),
            expect_equal(
                "scala CLI aspect clears the CLI gap",
                support_scala_cli_error(["@rules_dx//quality:real_aspects.bzl%real_scala_format_aspect"]),
                "support: scalafix lint is qualified but has no CLI lint aspect (missing real_scala_lint_aspect)",
            ),
            expect_equal(
                "both scala CLI aspects clear the CLI gap",
                support_scala_cli_error(["@rules_dx//quality:real_aspects.bzl%real_scala_format_aspect", "@rules_dx//quality:real_aspects.bzl%real_scala_lint_aspect"]),
                "",
            ),
            expect_equal(
                "scalafix is a target-coupled support tool with lint and fix",
                [is_support_tool("scalafix"), support_state("scalafix"), support_record_error(SCALA_SCALAFIX_SUPPORT), SCALA_SCALAFIX_SUPPORT["context"], SCALA_SCALAFIX_SUPPORT["capability"], SCALA_SCALAFIX_SUPPORT["fix"]],
                [True, "supported", "", "target_coupled", "lint", "supported"],
            ),
            expect_equal(
                "pinned scala artifact clears the artifact gap",
                support_scala_artifact_error(),
                "",
            ),
            expect_equal(
                "pinned shell artifacts clear the artifact gap",
                support_shell_artifact_error(),
                "",
            ),
            expect_equal(
                "terraform record validates and is qualified",
                [support_record_error(TERRAFORM_FORMAT_SUPPORT), is_support_tool("terraform"), support_state("terraform")],
                ["", True, "qualified"],
            ),
            expect_equal(
                "terraform record keeps direct single format with fix",
                [TERRAFORM_FORMAT_SUPPORT["capability"], TERRAFORM_FORMAT_SUPPORT["execution"], TERRAFORM_FORMAT_SUPPORT["transport"], TERRAFORM_FORMAT_SUPPORT["fix"]],
                ["format", "direct", "single", "supported"],
            ),
            expect_equal(
                "missing terraform aspect wiring is an actionable failure",
                support_terraform_wiring_error([]),
                "support: terraform format is qualified but has no wired aspect (missing real_terraform_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired terraform clears the aspect gap",
                support_terraform_wiring_error(["terraform"]),
                "",
            ),
            expect_equal(
                "missing terraform CLI aspect is an actionable failure",
                support_terraform_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: terraform format is qualified but has no CLI format aspect (missing real_terraform_format_aspect)",
            ),
            expect_equal(
                "terraform CLI aspect clears the CLI gap",
                support_terraform_cli_error(["@rules_dx//quality:real_aspects.bzl%real_terraform_format_aspect"]),
                "",
            ),
            expect_equal(
                "gofumpt record validates as a direct batched go format with fix",
                [support_record_error(GO_GOFUMPT_FORMAT_SUPPORT), is_support_tool("gofumpt"), support_state("gofumpt"), GO_GOFUMPT_FORMAT_SUPPORT["family"], GO_GOFUMPT_FORMAT_SUPPORT["execution"], GO_GOFUMPT_FORMAT_SUPPORT["transport"], GO_GOFUMPT_FORMAT_SUPPORT["fix"]],
                ["", True, "qualified", "go", "direct", "batched", "supported"],
            ),
            expect_equal(
                "missing gofumpt aspect wiring is an actionable failure",
                support_go_wiring_error([]),
                "support: gofumpt format is qualified but has no wired aspect (missing real_go_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired gofumpt clears the aspect gap",
                support_go_wiring_error(real_wired_tools()),
                "",
            ),
            expect_equal(
                "missing gofumpt CLI aspect is an actionable failure",
                support_go_cli_error(["@rules_dx//quality:real_aspects.bzl%real_format_aspect"]),
                "support: gofumpt format is qualified but has no CLI format aspect (missing real_go_format_aspect)",
            ),
            expect_equal(
                "go CLI aspect clears the CLI gap",
                support_go_cli_error(["@rules_dx//quality:real_aspects.bzl%real_go_format_aspect"]),
                "",
            ),
            expect_equal(
                "pinned gofumpt artifact clears the artifact gap",
                support_go_artifact_error(),
                "",
            ),
            expect_equal(
                "cue record validates as a direct batched format with fix",
                [support_record_error(CUE_FORMAT_SUPPORT), is_support_tool("cue"), support_state("cue"), CUE_FORMAT_SUPPORT["execution"], CUE_FORMAT_SUPPORT["transport"], CUE_FORMAT_SUPPORT["fix"], CUE_FORMAT_SUPPORT["platforms"]],
                ["", True, "qualified", "direct", "batched", "supported", ["linux_x86_64"]],
            ),
            expect_equal(
                "missing cue aspect wiring is an actionable failure",
                support_cue_wiring_error([]),
                "support: cue format is qualified but has no wired aspect (missing real_cue_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired cue clears the aspect gap",
                support_cue_wiring_error(["cue"]),
                "",
            ),
            expect_equal(
                "missing cue CLI aspect is an actionable failure",
                support_cue_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: cue format is qualified but has no CLI format aspect (missing real_cue_format_aspect)",
            ),
            expect_equal(
                "cue CLI aspect clears the CLI gap",
                support_cue_cli_error(["@rules_dx//quality:real_aspects.bzl%real_cue_format_aspect"]),
                "",
            ),
            expect_equal(
                "pinned cue artifact clears the artifact gap",
                support_cue_artifact_error(),
                "",
            ),
            expect_equal(
                "jsonnetfmt record validates as a direct batched format with fix",
                [support_record_error(JSONNET_FORMAT_SUPPORT), is_support_tool("jsonnetfmt"), support_state("jsonnetfmt"), JSONNET_FORMAT_SUPPORT["execution"], JSONNET_FORMAT_SUPPORT["transport"], JSONNET_FORMAT_SUPPORT["fix"], JSONNET_FORMAT_SUPPORT["platforms"]],
                ["", True, "qualified", "direct", "batched", "supported", ["linux_x86_64"]],
            ),
            expect_equal(
                "missing jsonnetfmt aspect wiring is an actionable failure",
                support_jsonnet_wiring_error([]),
                "support: jsonnetfmt format is qualified but has no wired aspect (missing real_jsonnet_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired jsonnetfmt clears the aspect gap",
                support_jsonnet_wiring_error(["jsonnetfmt"]),
                "",
            ),
            expect_equal(
                "missing jsonnetfmt CLI aspect is an actionable failure",
                support_jsonnet_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: jsonnetfmt format is qualified but has no CLI format aspect (missing real_jsonnet_format_aspect)",
            ),
            expect_equal(
                "jsonnetfmt CLI aspect clears the CLI gap",
                support_jsonnet_cli_error(["@rules_dx//quality:real_aspects.bzl%real_jsonnet_format_aspect"]),
                "",
            ),
            expect_equal(
                "pinned jsonnetfmt artifact clears the artifact gap",
                support_jsonnet_artifact_error(),
                "",
            ),
            expect_equal(
                "pkl record validates as a direct batched format with fix",
                [support_record_error(PKL_FORMAT_SUPPORT), is_support_tool("pkl"), support_state("pkl"), PKL_FORMAT_SUPPORT["execution"], PKL_FORMAT_SUPPORT["transport"], PKL_FORMAT_SUPPORT["fix"], PKL_FORMAT_SUPPORT["platforms"]],
                ["", True, "qualified", "direct", "batched", "supported", ["linux_x86_64"]],
            ),
            expect_equal(
                "missing pkl aspect wiring is an actionable failure",
                support_pkl_wiring_error([]),
                "support: pkl format is qualified but has no wired aspect (missing real_pkl_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired pkl clears the aspect gap",
                support_pkl_wiring_error(["pkl"]),
                "",
            ),
            expect_equal(
                "missing pkl CLI aspect is an actionable failure",
                support_pkl_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: pkl format is qualified but has no CLI format aspect (missing real_pkl_format_aspect)",
            ),
            expect_equal(
                "pkl CLI aspect clears the CLI gap",
                support_pkl_cli_error(["@rules_dx//quality:real_aspects.bzl%real_pkl_format_aspect"]),
                "",
            ),
            expect_equal(
                "pinned pkl artifact clears the artifact gap",
                support_pkl_artifact_error(),
                "",
            ),
            expect_equal(
                "csharpier record validates as a direct batched format with fix",
                [support_record_error(CSHARP_CSHARPIER_SUPPORT), is_support_tool("csharpier"), support_state("csharpier"), CSHARP_CSHARPIER_SUPPORT["execution"], CSHARP_CSHARPIER_SUPPORT["transport"], CSHARP_CSHARPIER_SUPPORT["fix"], CSHARP_CSHARPIER_SUPPORT["platforms"]],
                ["", True, "supported", "direct", "batched", "supported", ["linux_x86_64"]],
            ),
            expect_equal(
                "missing csharpier aspect wiring is an actionable failure",
                support_csharp_wiring_error([]),
                "support: csharpier format is qualified but has no wired aspect (missing real_dotnet_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired csharpier clears the aspect gap",
                support_csharp_wiring_error(["csharpier", "roslyn"]),
                "",
            ),
            expect_equal(
                "csharpier alone leaves roslyn unwired",
                support_csharp_wiring_error(["csharpier"]),
                "support: roslyn lint is qualified but has no wired aspect (missing real_dotnet_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "roslyn record validates as a direct batched check-only lint",
                [support_record_error(CSHARP_ROSLYN_SUPPORT), is_support_tool("roslyn"), support_state("roslyn"), CSHARP_ROSLYN_SUPPORT["execution"], CSHARP_ROSLYN_SUPPORT["transport"], CSHARP_ROSLYN_SUPPORT["fix"], CSHARP_ROSLYN_SUPPORT["context"]],
                ["", True, "supported", "direct", "batched", "check_only", "target_coupled"],
            ),
            expect_equal(
                "the real aspects wire csharpier and roslyn",
                support_csharp_wiring_error(real_wired_tools()),
                "",
            ),
            expect_equal(
                "missing csharpier CLI aspect is an actionable failure",
                support_csharp_cli_error(["@rules_dx//quality:real_aspects.bzl%real_scala_format_aspect"]),
                "support: csharpier format is qualified but has no CLI format aspect (missing real_dotnet_format_aspect)",
            ),
            expect_equal(
                "csharpier CLI aspect clears the CLI gap",
                support_csharp_cli_error(["@rules_dx//quality:real_aspects.bzl%real_dotnet_format_aspect", "@rules_dx//quality:real_aspects.bzl%real_dotnet_lint_aspect"]),
                "",
            ),
            expect_equal(
                "missing roslyn CLI aspect is an actionable failure",
                support_csharp_cli_error(["@rules_dx//quality:real_aspects.bzl%real_dotnet_format_aspect"]),
                "support: roslyn lint is qualified but has no CLI lint aspect (missing real_dotnet_lint_aspect)",
            ),
            expect_equal(
                "pinned csharpier artifact clears the artifact gap",
                support_csharp_artifact_error(),
                "",
            ),
            expect_equal(
                "fantomas and fsharplint records validate as direct batched tools",
                [support_record_error(FSHARP_FANTOMAS_SUPPORT), support_record_error(FSHARP_FSHARPLINT_SUPPORT), is_support_tool("fantomas"), is_support_tool("fsharplint"), support_state("fantomas"), support_state("fsharplint"), FSHARP_FANTOMAS_SUPPORT["fix"], FSHARP_FSHARPLINT_SUPPORT["fix"]],
                ["", "", True, True, "supported", "supported", "supported", "check_only"],
            ),
            expect_equal(
                "the real aspects wire fantomas and fsharplint",
                support_fsharp_wiring_error(real_wired_tools()),
                "",
            ),
            expect_equal(
                "dropping fantomas from the real wiring is an actionable failure",
                support_fsharp_wiring_error([tool for tool in real_wired_tools() if tool != "fantomas"]),
                "support: fantomas format is qualified but has no wired aspect (missing real_dotnet_format_aspect; not silently clean)",
            ),
            expect_equal(
                "dropping fsharplint from the real wiring is an actionable failure",
                support_fsharp_wiring_error([tool for tool in real_wired_tools() if tool != "fsharplint"]),
                "support: fsharplint lint is qualified but has no wired aspect (missing real_dotnet_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "pinned fsharp artifacts clear the artifact gap",
                support_fsharp_artifact_error(),
                "",
            ),
            expect_equal(
                "rubocop and standardrb records validate as direct batched tools",
                [support_record_error(RUBY_RUBOCOP_SUPPORT), support_record_error(RUBY_STANDARDRB_SUPPORT), is_support_tool("rubocop"), is_support_tool("standardrb"), support_state("rubocop"), support_state("standardrb"), RUBY_RUBOCOP_SUPPORT["fix"], RUBY_STANDARDRB_SUPPORT["fix"], RUBY_RUBOCOP_SUPPORT["platforms"], RUBY_STANDARDRB_SUPPORT["platforms"]],
                ["", "", True, True, "supported", "supported", "check_only", "supported", ["linux_x86_64"], ["linux_x86_64"]],
            ),
            expect_equal(
                "the real aspects wire rubocop and standardrb",
                support_ruby_wiring_error(real_wired_tools()),
                "",
            ),
            expect_equal(
                "dropping rubocop from the real wiring is an actionable failure",
                support_ruby_wiring_error([tool for tool in real_wired_tools() if tool != "rubocop"]),
                "support: rubocop lint is qualified but has no wired aspect (missing real_ruby_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "dropping standardrb from the real wiring is an actionable failure",
                support_ruby_wiring_error([tool for tool in real_wired_tools() if tool != "standardrb"]),
                "support: standardrb format is qualified but has no wired aspect (missing real_ruby_format_aspect; not silently clean)",
            ),
            expect_equal(
                "pinned ruby artifacts clear the artifact gap",
                support_ruby_artifact_error(),
                "",
            ),
            expect_equal(
                "djlint records validate as direct single-file tools",
                [support_record_error(HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT), support_record_error(HTML_TEMPLATE_DJLINT_LINT_SUPPORT), is_support_tool("djlint"), support_state("djlint"), HTML_TEMPLATE_DJLINT_FORMAT_SUPPORT["fix"], HTML_TEMPLATE_DJLINT_LINT_SUPPORT["fix"], HTML_TEMPLATE_DJLINT_LINT_SUPPORT["platforms"]],
                ["", "", True, "supported", "supported", "check_only", ["linux_x86_64"]],
            ),
            expect_equal(
                "the real aspects wire djlint",
                support_html_template_wiring_error(real_wired_tools()),
                "",
            ),
            expect_equal(
                "dropping djlint from the real wiring is an actionable failure",
                support_html_template_wiring_error([tool for tool in real_wired_tools() if tool != "djlint"]),
                "support: djlint format is qualified but has no wired aspect (missing real_html_template_format_aspect; not silently clean)",
            ),
            expect_equal(
                "missing html template CLI aspects are an actionable failure",
                support_html_template_cli_error(["@rules_dx//quality:real_aspects.bzl%real_html_template_lint_aspect"]),
                "support: djlint format is qualified but has no CLI format aspect (missing real_html_template_format_aspect)",
            ),
            expect_equal(
                "html template CLI aspects clear the CLI gap",
                support_html_template_cli_error(["@rules_dx//quality:real_aspects.bzl%real_html_template_format_aspect", "@rules_dx//quality:real_aspects.bzl%real_html_template_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned djlint artifact clears the artifact gap",
                support_html_template_artifact_error(),
                "",
            ),
            expect_equal(
                "yamlfmt and yamllint records validate as direct batched tools",
                [support_record_error(YAML_YAMLFMT_SUPPORT), support_record_error(YAML_YAMLLINT_SUPPORT), is_support_tool("yamlfmt"), is_support_tool("yamllint"), support_state("yamlfmt"), support_state("yamllint"), YAML_YAMLFMT_SUPPORT["fix"], YAML_YAMLLINT_SUPPORT["fix"], YAML_YAMLFMT_SUPPORT["platforms"], YAML_YAMLLINT_SUPPORT["platforms"]],
                ["", "", True, True, "supported", "supported", "supported", "check_only", ["linux_x86_64"], ["linux_x86_64"]],
            ),
            expect_equal(
                "the real aspects wire yamlfmt and yamllint",
                support_yaml_wiring_error(real_wired_tools()),
                "",
            ),
            expect_equal(
                "dropping yamlfmt from the real wiring is an actionable failure",
                support_yaml_wiring_error([tool for tool in real_wired_tools() if tool != "yamlfmt"]),
                "support: yamlfmt format is qualified but has no wired aspect (missing real_yaml_format_aspect; not silently clean)",
            ),
            expect_equal(
                "dropping yamllint from the real wiring is an actionable failure",
                support_yaml_wiring_error([tool for tool in real_wired_tools() if tool != "yamllint"]),
                "support: yamllint lint is qualified but has no wired aspect (missing real_yaml_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "missing yaml CLI aspects are an actionable failure",
                support_yaml_cli_error(["@rules_dx//quality:real_aspects.bzl%real_yaml_lint_aspect"]),
                "support: yamlfmt format is qualified but has no CLI format aspect (missing real_yaml_format_aspect)",
            ),
            expect_equal(
                "yaml CLI aspects clear the CLI gap",
                support_yaml_cli_error(["@rules_dx//quality:real_aspects.bzl%real_yaml_format_aspect", "@rules_dx//quality:real_aspects.bzl%real_yaml_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned yaml artifacts clear the artifact gap",
                support_yaml_artifact_error(),
                "",
            ),
            expect_equal(
                "pinned terraform artifact clears the artifact gap",
                support_terraform_artifact_error(),
                "",
            ),
            expect_equal(
                "buf records validate as direct batched format and lint",
                [support_record_error(PROTOBUF_BUF_FORMAT_SUPPORT), support_record_error(PROTOBUF_BUF_LINT_SUPPORT), is_support_tool("buf"), support_state("buf"), PROTOBUF_BUF_FORMAT_SUPPORT["fix"], PROTOBUF_BUF_LINT_SUPPORT["fix"]],
                ["", "", True, "supported", "supported", "check_only"],
            ),
            expect_equal(
                "missing buf aspect wiring is an actionable failure",
                support_protobuf_wiring_error([]),
                "support: buf format is qualified but has no wired aspect (missing real_protobuf_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired buf clears the aspect gap",
                support_protobuf_wiring_error(["buf"]),
                "",
            ),
            expect_equal(
                "missing buf CLI aspect is an actionable failure",
                support_protobuf_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: buf format is qualified but has no CLI format aspect (missing real_protobuf_format_aspect)",
            ),
            expect_equal(
                "buf CLI aspects clear the CLI gap",
                support_protobuf_cli_error(["@rules_dx//quality:real_aspects.bzl%real_protobuf_format_aspect", "@rules_dx//quality:real_aspects.bzl%real_protobuf_lint_aspect"]),
                "",
            ),
            expect_equal(
                "stylelint record validates as direct batched check-only lint",
                [support_record_error(CSS_STYLELINT_LINT_SUPPORT), is_support_tool("stylelint"), support_state("stylelint"), CSS_STYLELINT_LINT_SUPPORT["fix"]],
                ["", True, "supported", "check_only"],
            ),
            expect_equal(
                "missing stylelint aspect wiring is an actionable failure",
                support_css_wiring_error([]),
                "support: stylelint lint is qualified but has no wired aspect (missing real_js_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "wired stylelint clears the aspect gap",
                support_css_wiring_error(["stylelint"]),
                "",
            ),
            expect_equal(
                "missing stylelint CLI aspect is an actionable failure",
                support_css_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: stylelint lint is qualified but has no CLI lint aspect (missing real_js_lint_aspect)",
            ),
            expect_equal(
                "stylelint CLI aspect clears the CLI gap",
                support_css_cli_error(["@rules_dx//quality:real_aspects.bzl%real_js_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned stylelint artifact clears the artifact gap",
                support_css_artifact_error(),
                "",
            ),
            expect_equal(
                "graphql_eslint record validates as direct batched check-only GraphQL lint",
                [support_record_error(GRAPHQL_ESLINT_LINT_SUPPORT), is_support_tool("graphql_eslint"), support_state("graphql_eslint"), GRAPHQL_ESLINT_LINT_SUPPORT["fix"], GRAPHQL_ESLINT_LINT_SUPPORT["family"], GRAPHQL_ESLINT_LINT_SUPPORT["platforms"]],
                ["", True, "supported", "check_only", "graphql", ["linux_x86_64"]],
            ),
            expect_equal(
                "missing graphql_eslint aspect wiring is an actionable failure",
                support_graphql_wiring_error([]),
                "support: graphql_eslint lint is qualified but has no wired aspect (missing real_js_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "wired graphql_eslint clears the aspect gap",
                support_graphql_wiring_error(["graphql_eslint"]),
                "",
            ),
            expect_equal(
                "missing graphql_eslint CLI aspect is an actionable failure",
                support_graphql_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: graphql_eslint lint is qualified but has no CLI lint aspect (missing real_js_lint_aspect)",
            ),
            expect_equal(
                "graphql_eslint CLI aspect clears the CLI gap",
                support_graphql_cli_error(["@rules_dx//quality:real_aspects.bzl%real_js_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned graphql_eslint artifact clears the artifact gap",
                support_graphql_artifact_error(),
                "",
            ),
            expect_equal(
                "redocly record validates as direct single-root check-only API spec lint",
                [support_record_error(API_SPEC_REDOCLY_LINT_SUPPORT), is_support_tool("redocly"), support_state("redocly"), API_SPEC_REDOCLY_LINT_SUPPORT["fix"], API_SPEC_REDOCLY_LINT_SUPPORT["family"], API_SPEC_REDOCLY_LINT_SUPPORT["transport"], API_SPEC_REDOCLY_LINT_SUPPORT["platforms"]],
                ["", True, "supported", "check_only", "api_spec", "single", ["linux_x86_64"]],
            ),
            expect_equal(
                "missing redocly aspect wiring is an actionable failure",
                support_api_spec_wiring_error([]),
                "support: redocly lint is qualified but has no wired aspect (missing real_js_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "wired redocly clears the aspect gap",
                support_api_spec_wiring_error(["redocly"]),
                "",
            ),
            expect_equal(
                "missing redocly CLI aspect is an actionable failure",
                support_api_spec_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: redocly lint is qualified but has no CLI lint aspect (missing real_js_lint_aspect)",
            ),
            expect_equal(
                "redocly CLI aspect clears the CLI gap",
                support_api_spec_cli_error(["@rules_dx//quality:real_aspects.bzl%real_js_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned redocly artifact clears the artifact gap",
                support_api_spec_artifact_error(),
                "",
            ),
            expect_equal(
                "remark record validates as direct batched fixable mdx format",
                [support_record_error(MDX_REMARK_FORMAT_SUPPORT), is_support_tool("remark"), support_state("remark"), MDX_REMARK_FORMAT_SUPPORT["fix"], MDX_REMARK_FORMAT_SUPPORT["classes"]],
                ["", True, "supported", "supported", ["mdx"]],
            ),
            expect_equal(
                "missing remark aspect wiring is an actionable failure",
                support_mdx_wiring_error([]),
                "support: remark format is qualified but has no wired aspect (missing real_js_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired remark clears the aspect gap",
                support_mdx_wiring_error(["remark"]),
                "",
            ),
            expect_equal(
                "missing remark CLI aspect is an actionable failure",
                support_mdx_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: remark format is qualified but has no CLI format aspect (missing real_js_format_aspect)",
            ),
            expect_equal(
                "remark CLI aspect clears the CLI gap",
                support_mdx_cli_error(["@rules_dx//quality:real_aspects.bzl%real_js_format_aspect"]),
                "",
            ),
            expect_equal(
                "pinned remark artifact clears the artifact gap",
                support_mdx_artifact_error(),
                "",
            ),
            expect_equal(
                "prettier plugin records validate as direct batched fixable format",
                [[support_record_error(r) for r in [SQL_PRETTIER_FORMAT_SUPPORT, XML_PRETTIER_FORMAT_SUPPORT, GHERKIN_PRETTIER_FORMAT_SUPPORT]], [r["fix"] for r in [SQL_PRETTIER_FORMAT_SUPPORT, XML_PRETTIER_FORMAT_SUPPORT, GHERKIN_PRETTIER_FORMAT_SUPPORT]], [r["family"] for r in [SQL_PRETTIER_FORMAT_SUPPORT, XML_PRETTIER_FORMAT_SUPPORT, GHERKIN_PRETTIER_FORMAT_SUPPORT]], support_state("prettier")],
                [["", "", ""], ["supported", "supported", "supported"], ["sql", "xml", "gherkin"], "supported"],
            ),
            expect_equal(
                "missing prettier aspect wiring is an actionable failure",
                support_prettier_plugins_wiring_error([]),
                "support: prettier format is qualified but has no wired aspect (missing real_js_format_aspect; not silently clean)",
            ),
            expect_equal(
                "wired prettier clears the plugin aspect gap",
                support_prettier_plugins_wiring_error(["prettier"]),
                "",
            ),
            expect_equal(
                "missing prettier CLI aspect is an actionable failure",
                support_prettier_plugins_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: prettier format is qualified but has no CLI format aspect (missing real_js_format_aspect)",
            ),
            expect_equal(
                "prettier CLI aspect clears the plugin CLI gap",
                support_prettier_plugins_cli_error(["@rules_dx//quality:real_aspects.bzl%real_js_format_aspect"]),
                "",
            ),
            expect_equal(
                "pinned prettier plugins clear the artifact gap",
                support_prettier_plugins_artifact_error(),
                "",
            ),
            expect_equal(
                "psscriptanalyzer record validates as direct batched check-only lint",
                [support_record_error(POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT), is_support_tool("psscriptanalyzer"), support_state("psscriptanalyzer"), POWERSHELL_PSSCRIPTANALYZER_LINT_SUPPORT["fix"]],
                ["", True, "supported", "check_only"],
            ),
            expect_equal(
                "missing psscriptanalyzer aspect wiring is an actionable failure",
                support_powershell_wiring_error([]),
                "support: psscriptanalyzer lint is qualified but has no wired aspect (missing real_powershell_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "wired psscriptanalyzer clears the aspect gap",
                support_powershell_wiring_error(["psscriptanalyzer"]),
                "",
            ),
            expect_equal(
                "missing psscriptanalyzer CLI aspect is an actionable failure",
                support_powershell_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_format_aspect"]),
                "support: psscriptanalyzer lint is qualified but has no CLI lint aspect (missing real_powershell_lint_aspect)",
            ),
            expect_equal(
                "psscriptanalyzer CLI aspect clears the CLI gap",
                support_powershell_cli_error(["@rules_dx//quality:real_aspects.bzl%real_powershell_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned psscriptanalyzer artifact clears the artifact gap",
                support_powershell_artifact_error(),
                "",
            ),
            expect_equal(
                "hadolint record validates as direct batched check-only lint on four platforms",
                [support_record_error(DOCKERFILE_HADOLINT_SUPPORT), is_support_tool("hadolint"), support_state("hadolint"), DOCKERFILE_HADOLINT_SUPPORT["execution"], DOCKERFILE_HADOLINT_SUPPORT["transport"], DOCKERFILE_HADOLINT_SUPPORT["fix"], DOCKERFILE_HADOLINT_SUPPORT["platforms"]],
                ["", True, "supported", "direct", "batched", "check_only", ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"]],
            ),
            expect_equal(
                "missing hadolint aspect wiring is an actionable failure",
                support_dockerfile_wiring_error([]),
                "support: hadolint lint is qualified but has no wired aspect (missing real_dockerfile_lint_aspect; not silently clean)",
            ),
            expect_equal(
                "wired hadolint clears the aspect gap",
                support_dockerfile_wiring_error(["hadolint"]),
                "",
            ),
            expect_equal(
                "missing hadolint CLI aspect is an actionable failure",
                support_dockerfile_cli_error(["@rules_dx//quality:real_aspects.bzl%real_shell_lint_aspect"]),
                "support: hadolint lint is qualified but has no CLI lint aspect (missing real_dockerfile_lint_aspect)",
            ),
            expect_equal(
                "hadolint CLI aspect clears the CLI gap",
                support_dockerfile_cli_error(["@rules_dx//quality:real_aspects.bzl%real_dockerfile_lint_aspect"]),
                "",
            ),
            expect_equal(
                "pinned hadolint artifact clears the artifact gap",
                support_dockerfile_artifact_error(),
                "",
            ),
            expect_equal(
                "pinned buf artifact clears the artifact gap",
                support_protobuf_artifact_error(),
                "",
            ),
        ],
    )
