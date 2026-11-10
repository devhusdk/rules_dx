"""Matrix cases: every tier file is already formatted, so a format run changes nothing."""

load("//quality/tiers:corpora.bzl", "TIER_JAVASCRIPT", "TIER_JSON", "TIER_TOML", "TIER_YAML", "tier_labels")

def _fixpoint_case(name, tool, binary, source_class, paths, config = None):
    sources = ",".join(["quality/tiers/" + path for path in paths])
    case = {
        "name": name,
        "srcs": tier_labels(paths),
        "capability": "format",
        "stages": ["%s;%s;%s" % (tool, source_class, sources)],
        "tool_names": [tool],
        "tool_binaries": [binary],
        "expected": "\n".join(
[
            "producer //quality/testdata:" + name,
            "capability FORMAT",
            "stages 1",
            "stage %s classes=%s sources=%s" % (tool, source_class, sources),
            "completed_rounds 1",
            "convergence STABLE",
            "initial 0",
            "terminal 0",
            "replacements 0",
            "",
        ]
        ),
    }
    if config:
        case["toolfile_tools"] = [tool]
        case["toolfile_srcs"] = [config]
        case["config_tools"] = [tool]
        case["config_files"] = [config]
    return case

_TFLINT_HEADER = "terraform {\n  required_version = \">= 1.5\"\n  required_providers {\n    null = {\n      source  = \"hashicorp/null\"\n      version = \">= 3.0\"\n    }\n  }\n}\n\n"

_HADOLINT_CLEAN = {"matrix/hadolint_tier/Dockerfile": "FROM scratch\nCOPY hello /hello\nENTRYPOINT [\"/hello\"]\n"}
_HADOLINT_STYLE = {"matrix/hadolint_style/Dockerfile": "FROM scratch\nRUN [\"/bin/a\"]\nRUN [\"/bin/b\"]\n"}
_TFLINT_DOCUMENTED = {
    "matrix/tflint_tier/variables.tf": "variable \"name\" {\n  type        = string\n  description = \"Name.\"\n}\n",
    "matrix/tflint_tier/main.tf": _TFLINT_HEADER + "resource \"null_resource\" \"sample\" {\n  triggers = {\n    name = var.name\n  }\n}\n",
}
_SELENE_CLEAN = {"matrix/selene_tier/add.lua": "local function add(a, b)\n    return a + b\nend\n\nprint(add(1, 2))\n"}
_DART_CLEAN = {"matrix/dart_tier/main.dart": "void main() {\n  print(1);\n}\n"}
_SWIFT_CLEAN = {"matrix/swift_lint_tier/add.swift": "func add(a: Int, b: Int) -> Int {\n  return a + b\n}\n"}
_NO_PLATFORMS = {"linux_arm64": False, "macos_arm64": False, "windows_x86_64": False, "windows_arm64": False}
_TRIVY_CLEAN = {"matrix/trivy_tier/Dockerfile": "FROM scratch\nUSER 1000\nHEALTHCHECK NONE\n"}

def _lint_case(name, generated, stage, tool, binary, config, **extra):
    case = {
        "name": name,
        "generated": generated,
        "capability": "lint",
        "stages": [stage],
        "tool_names": [tool],
        "tool_binaries": [binary],
        "toolfile_tools": [tool],
        "toolfile_srcs": [config],
        "config_tools": [tool],
        "config_files": [config],
        "expected_file": ":matrix/%s.expected.txt" % name,
        "linux_arm64": True,
        "macos_arm64": True,
        "windows_x86_64": True,
        "windows_arm64": True,
    }
    case.update(extra)
    return case

def _lint_cases(tier):
    return [
        _lint_case(
            "matrix_dockerfile_lint_clean_" + tier,
            _HADOLINT_CLEAN,
            "hadolint;dockerfile;matrix/hadolint_tier/Dockerfile",
            "hadolint",
            "@dx_tools//:hadolint",
            "//quality/tiers/hadolint:%s.yaml" % tier,
        ),
        _lint_case(
            "matrix_dockerfile_lint_style_" + tier,
            _HADOLINT_STYLE,
            "hadolint;dockerfile;matrix/hadolint_style/Dockerfile",
            "hadolint",
            "@dx_tools//:hadolint",
            "//quality/tiers/hadolint:%s.yaml" % tier,
        ),
        _lint_case(
            "matrix_terraform_lint_clean_" + tier,
            _TFLINT_DOCUMENTED,
            "tflint;terraform;matrix/tflint_tier/main.tf,matrix/tflint_tier/variables.tf",
            "tflint",
            "@dx_tools//:tflint",
            "//quality/tiers/tflint:%s.hcl" % tier,
            tool_env = ["tflint=CHECKPOINT_DISABLE=1"],
        ),
        _lint_case(
            "matrix_dockerfile_trivy_lint_clean_" + tier,
            _TRIVY_CLEAN,
            "trivy_config;dockerfile;matrix/trivy_tier/Dockerfile",
            "trivy_config",
            "@dx_tools//:trivy",
            "//quality/tiers/trivy_config:%s.txt" % tier,
            toolfile_rels = [""],
        ),
        _lint_case(
            "matrix_lua_selene_clean_" + tier,
            _SELENE_CLEAN,
            "selene;lua;matrix/selene_tier/add.lua",
            "selene",
            "//rust/private/quality/tools:selene",
            "//quality/tiers/selene:%s.toml" % tier,
            **_NO_PLATFORMS
        ),
        _lint_case(
            "matrix_dart_lint_clean_" + tier,
            _DART_CLEAN,
            "dart;dart;matrix/dart_tier/main.dart",
            "dart",
            "//dart/private/tools:dart",
            "//quality/tiers/dart:%s.yaml" % tier,
            toolfile_tools = ["dart", "dart"],
            toolfile_srcs = ["//quality/tiers/dart:%s.yaml" % tier, ":dart_cfg/package_config.json"],
            toolfile_rels = ["analysis_options.yaml", ".dart_tool/package_config.json"],
            windows_arm64 = False,
        ),
        _lint_case(
            "matrix_swift_lint_clean_" + tier,
            _SWIFT_CLEAN,
            "swift_lint;swift;matrix/swift_lint_tier/add.swift",
            "swift_lint",
            "//swift/private/tools:swift_format",
            "//quality/tiers/swift_lint:%s.json" % tier,
            **_NO_PLATFORMS
        ),
    ]

TIERS_CASES = _lint_cases("standard") + _lint_cases("strict") + [
    _fixpoint_case("matrix_tiers_javascript_format_pass", "biome", "@dx_tools//:biome", "javascript", TIER_JAVASCRIPT, "//:biome.json"),
    _fixpoint_case("matrix_tiers_json_format_pass", "prettier", "//javascript/private/quality/bin:prettier", "json", TIER_JSON),
    _fixpoint_case("matrix_tiers_toml_format_pass", "taplo", "@dx_tools//:taplo", "toml", TIER_TOML),
    _fixpoint_case("matrix_tiers_yaml_format_pass", "yamlfmt", "@dx_tools//:yamlfmt", "yaml", TIER_YAML),
]
