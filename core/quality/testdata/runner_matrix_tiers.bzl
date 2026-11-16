"""Matrix cases: every tier file is already formatted, so a format run changes nothing."""

load(":runner_matrix_file_family.bzl", "TFLINT_HEADER")
load(":runner_matrix_tests.bzl", "matrix_case")
load("//core/quality/tiers:corpora.bzl", "TIER_JAVASCRIPT", "TIER_JSON", "TIER_TOML", "TIER_YAML", "tier_labels")

def _fixpoint_case(name, tool, binary, source_class, paths, config = None):
    sources = ",".join(["core/quality/tiers/" + path for path in paths])
    case = {
        "name": name,
        "srcs": tier_labels(paths),
        "capability": "format",
        "stages": ["%s;%s;%s" % (tool, source_class, sources)],
        "tool_names": [tool],
        "tool_binaries": [binary],
        "expected": "\n".join(
[
            "producer //core/quality/testdata:" + name,
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

_HADOLINT_CLEAN = {"matrix/hadolint_tier/Dockerfile": "FROM scratch\nCOPY hello /hello\nENTRYPOINT [\"/hello\"]\n"}
_HADOLINT_STYLE = {"matrix/hadolint_style/Dockerfile": "FROM scratch\nRUN [\"/bin/a\"]\nRUN [\"/bin/b\"]\n"}
_TFLINT_DOCUMENTED = {
    "matrix/tflint_tier/variables.tf": "variable \"name\" {\n  type        = string\n  description = \"Name.\"\n}\n",
    "matrix/tflint_tier/main.tf": TFLINT_HEADER + "resource \"null_resource\" \"sample\" {\n  triggers = {\n    name = var.name\n  }\n}\n",
}
_SELENE_CLEAN = {"matrix/selene_tier/add.lua": "local function add(a, b)\n    return a + b\nend\n\nprint(add(1, 2))\n"}
_DART_CLEAN = {"matrix/dart_tier/main.dart": "void main() {\n  print(1);\n}\n"}
_SWIFT_CLEAN = {"matrix/swift_lint_tier/add.swift": "func add(a: Int, b: Int) -> Int {\n  return a + b\n}\n"}
_NO_PLATFORMS = ["linux_x86_64"]
_TRIVY_CLEAN = {"matrix/trivy_tier/Dockerfile": "FROM scratch\nUSER 1000\nHEALTHCHECK NONE\n"}

def _lint_case(name, generated, source_class, tool, binary, config, **extra):
    return matrix_case(
        name = name,
        capability = "lint",
        tool = tool,
        binary = binary,
        source_class = source_class,
        generated = generated,
        config = config,
        **extra
    )

def _lint_cases(tier):
    return [
        _lint_case(
            "matrix_dockerfile_lint_clean_" + tier,
            _HADOLINT_CLEAN,
            "dockerfile",
            "hadolint",
            "@dx_tools//:hadolint",
            "//core/quality/tiers/hadolint:%s.yaml" % tier,
        ),
        _lint_case(
            "matrix_dockerfile_lint_style_" + tier,
            _HADOLINT_STYLE,
            "dockerfile",
            "hadolint",
            "@dx_tools//:hadolint",
            "//core/quality/tiers/hadolint:%s.yaml" % tier,
        ),
        _lint_case(
            "matrix_terraform_lint_clean_" + tier,
            _TFLINT_DOCUMENTED,
            "terraform",
            "tflint",
            "@dx_tools//:tflint",
            "//core/quality/tiers/tflint:%s.hcl" % tier,
            tool_env = ["tflint=CHECKPOINT_DISABLE=1"],
        ),
        _lint_case(
            "matrix_dockerfile_trivy_lint_clean_" + tier,
            _TRIVY_CLEAN,
            "dockerfile",
            "trivy_config",
            "@dx_tools//:trivy",
            "//core/quality/tiers/trivy_config:%s.txt" % tier,
            toolfile_rels = [""],
        ),
        _lint_case(
            "matrix_lua_selene_clean_" + tier,
            _SELENE_CLEAN,
            "lua",
            "selene",
            "//rust/private/quality/tools:selene",
            "//core/quality/tiers/selene:%s.toml" % tier,
            platforms = _NO_PLATFORMS
        ),
        _lint_case(
            "matrix_dart_lint_clean_" + tier,
            _DART_CLEAN,
            "dart",
            "dart",
            "//dart/private/tools:dart",
            "//core/quality/tiers/dart:%s.yaml" % tier,
            toolfile_tools = ["dart", "dart"],
            toolfile_srcs = ["//core/quality/tiers/dart:%s.yaml" % tier, "//core/quality/tests/testdata:dart_cfg/package_config.json"],
            toolfile_rels = ["analysis_options.yaml", ".dart_tool/package_config.json"],
            platforms = ["linux_arm64", "macos_arm64", "windows_x86_64"],
        ),
        _lint_case(
            "matrix_swift_lint_clean_" + tier,
            _SWIFT_CLEAN,
            "swift",
            "swift_lint",
            "//swift/private/tools:swift_format",
            "//core/quality/tiers/swift_lint:%s.json" % tier,
            platforms = _NO_PLATFORMS
        ),
    ]

TIERS_CASES = _lint_cases("standard") + _lint_cases("strict") + [
    _fixpoint_case("matrix_tiers_javascript_format_pass", "biome", "@dx_tools//:biome", "javascript", TIER_JAVASCRIPT, "//:biome.json"),
    _fixpoint_case("matrix_tiers_json_format_pass", "prettier", "//javascript/private/quality/bin:prettier", "json", TIER_JSON),
    _fixpoint_case("matrix_tiers_toml_format_pass", "taplo", "@dx_tools//:taplo", "toml", TIER_TOML),
    _fixpoint_case("matrix_tiers_yaml_format_pass", "yamlfmt", "@dx_tools//:yamlfmt", "yaml", TIER_YAML),
]
