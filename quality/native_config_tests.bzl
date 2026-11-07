"""Unit tests for typed native-config validation."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(
    "//quality/private:native_config_impl.bzl",
    "CONFIG_REQUIRED_TOOLS",
    "CONFIG_TOOL_ALIASES",
    "DxNativeConfigInfo",
    "bound_config_targets",
    "collect_native_configs",
    "missing_required_config_error",
    "native_config_error",
    "native_config_extension",
    "native_config_extensions",
    "native_config_scratch_rel",
    "tier_base_dirs",
)

_HINTS = [
    struct(tool_id = "biome"),
    struct(tool_id = "buildifier"),
    struct(tool_id = "taplo"),
    struct(tool_id = "ruff"),
    struct(tool_id = "eslint"),
    struct(tool_id = "scalafmt"),
]

_TARGET_RUFF = {DxNativeConfigInfo: struct(tool_id = "ruff", name = "hint")}
_TARGET_POLICY_RUFF = {DxNativeConfigInfo: struct(tool_id = "ruff", name = "policy")}
_TARGET_DART = {DxNativeConfigInfo: struct(tool_id = "dart", name = "dart")}

def _names(targets):
    return {tool: t[DxNativeConfigInfo].name for tool, t in targets.items()}

# Tools whose runner refuses to run without a config; tools/ci/config_consistency_test.rs pins each to its refusal.
_RUNNER_CONFIG_REQUIRED_TOOLS = []

def native_config_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "tier_base_dirs binds file tools with tier targets only",
                [
                    [tool in tier_base_dirs() for tool in ["actionlint", "clippy", "prettier", "kubeconform", "bicep", "ruff"]],
                    len(tier_base_dirs()),
                ],
                [[False, False, False, False, True, True], 68],
            ),
            expect_equal(
                "native_config_scratch_rel keeps in-repo paths and remaps external ones",
                [
                    native_config_scratch_rel("pkg/.prettierrc.json"),
                    native_config_scratch_rel("../shared_prettier+/cfg/.prettierrc.json"),
                    native_config_scratch_rel("../+ext+x/a.json"),
                ],
                [
                    "pkg/.prettierrc.json",
                    "_external/shared_prettier+/cfg/.prettierrc.json",
                    "_external/+ext+x/a.json",
                ],
            ),
            expect_equal(
                "native_config_scratch_rel rejects remaining parent segments",
                [native_config_scratch_rel("../a/../../b"), native_config_scratch_rel("pkg/../../x")],
                [None, None],
            ),
            expect_equal(
                "native_config_extension pins the buildifier/biome JSON transport",
                [
                    native_config_extension("buildifier"),
                    native_config_extension("biome"),
                ],
                [".json", ".json"],
            ),
            expect_equal(
                "native_config_extension pins TOML for taplo/rustfmt/ruff",
                [
                    native_config_extension("taplo"),
                    native_config_extension("rustfmt"),
                    native_config_extension("ruff"),
                ],
                [".toml", ".toml", ".toml"],
            ),
            expect_equal(
                "native_config_extension pins the vale INI transport",
                native_config_extension("vale"),
                ".ini",
            ),
            expect_equal(
                "native_config_extension pins the eslint JS flat-config transport",
                native_config_extension("eslint"),
                ".js",
            ),
            expect_equal(
                "native_config_extensions pins the eslint module transports",
                native_config_extensions("eslint"),
                [".js", ".mjs", ".cjs"],
            ),
            expect_equal(
                "native_config_extension pins the Java XML transports",
                [native_config_extension("checkstyle"), native_config_extension("pmd")],
                [".xml", ".xml"],
            ),
            expect_equal(
                "native_config_error rejects a non-xml pmd ruleset",
                native_config_error("pmd", "cfg/pmd.yaml", True, []),
                "native_config (pmd): src must end in '.xml', got cfg/pmd.yaml",
            ),
            expect_equal(
                "native_config_error accepts an xml pmd ruleset",
                native_config_error("pmd", "cfg/pmd-ruleset.xml", True, []),
                "",
            ),
            expect_equal(
                "native_config_extension pins Scala/.NET transports",
                [
                    native_config_extension("scalafmt"),
                    native_config_extension("scalafix"),
                    native_config_extension("csharpier"),
                    native_config_extension("fsharplint"),
                ],
                [".conf", ".conf", ".yaml", ".json"],
            ),
            expect_equal(
                "native_config_extension pins native cohort transports",
                [
                    native_config_extension("clang_format"),
                    native_config_extension("clang_tidy"),
                    native_config_extension("cppcheck"),
                    native_config_extension("staticcheck"),
                ],
                [".clang-format", ".clang-tidy", ".txt", ".conf"],
            ),
            expect_equal(
                "native_config_extension pins Structured transports",
                [
                    native_config_extension("buf"),
                    native_config_extension("qmlformat"),
                    native_config_extension("qmllint"),
                ],
                [".yaml", ".qmlformat.ini", ".qmllint.ini"],
            ),
            expect_equal(
                "native_config_extension pins file-family transports",
                [
                    native_config_extension("stylelint"),
                    native_config_extension("djlint"),
                    native_config_extension("yamllint"),
                    native_config_extension("redocly"),
                    native_config_extension("remark"),
                ],
                [".json", "djlint.toml", ".yaml", ".yaml", ".json"],
            ),
            expect_equal(
                "native_config_extension pins the gplint JSON transport",
                native_config_extension("gplint"),
                ".gplintrc",
            ),
            expect_equal(
                "native_config_extension pins the graphql_eslint JSON transport",
                native_config_extension("graphql_eslint"),
                ".json",
            ),
            expect_equal(
                "native_config_extension pins the shellcheck rc suffix",
                native_config_extension("shellcheck"),
                ".shellcheckrc",
            ),
            expect_equal(
                "native_config_error accepts eslint module configs",
                [
                    native_config_error(
                        "eslint",
                        "cfg/eslint.config.mjs",
                        True,
                        [],
                    ),
                    native_config_error(
                        "eslint",
                        "cfg/eslint.config.cjs",
                        True,
                        [],
                    ),
                ],
                ["", ""],
            ),
            expect_equal(
                "native_config_error rejects an unsupported eslint transport",
                native_config_error(
                    "eslint",
                    "cfg/eslint.config.json",
                    True,
                    [],
                ),
                "native_config (eslint): src must end in one of " +
                "'.js', '.mjs', '.cjs', got cfg/eslint.config.json",
            ),
            expect_equal(
                "native_config_error rejects biome.jsonc",
                native_config_error(
                    "biome",
                    "cfg/biome.jsonc",
                    True,
                    [],
                ),
                "native_config (biome): src must end in '.json', got cfg/biome.jsonc",
            ),
            expect_equal(
                "native_config_error accepts a checked-in config",
                native_config_error(
                    "buildifier",
                    ".buildifier.json",
                    True,
                    [],
                ),
                "",
            ),
            expect_equal(
                "native_config_error accepts checked-in data",
                native_config_error(
                    "vale",
                    ".vale.ini",
                    True,
                    [struct(is_source = True, path = "styles/Test/Cotton.yml")],
                ),
                "",
            ),
            expect_equal(
                "native_config_error rejects an unknown tool",
                native_config_error("nope", "x.json", True, []),
                "native_config: unknown tool 'nope': want one of " +
                "ansible_lint, bicep, biome, buf, buildifier, cfn_lint, checkmake, checkstyle, clang_format, clang_tidy, cmakelint, cppcheck, credo, csharpier, dart, deadnix, detekt, djlint, elvis, errcheck, eslint, ffmt, fortitude, fsharplint, gersemi, govet, gplint, graphql_eslint, hadolint, hlint, julialint, kubeconform, latexindent, luals, lychee, markdown_check, mbake, mix_format, perlcritic, perltidy, phpcs, phpstan, pmd, prettier, psscriptanalyzer, pydoclint, qmlformat, qmllint, redocly, regal, remark, roslyn, rubocop, ruff, rustfmt, scalafix, scalafmt, selene, shellcheck, shfmt, spotbugs, sqlfluff, staticcheck, stylelint, stylua, swift_format, swift_lint, taplo, tflint, trivy_config, ty, typos, vale, yamlfmt, yamllint, zanuda, zizmor",
            ),
            expect_equal(
                "native_config_error requires a config",
                native_config_error("taplo", "", True, []),
                "native_config (taplo): src is required",
            ),
            expect_equal(
                "native_config_error rejects a generated config",
                native_config_error("taplo", "gen/taplo.toml", False, []),
                "native_config (taplo): src must be a checked-in source " +
                "file, got generated gen/taplo.toml",
            ),
            expect_equal(
                "native_config_error accepts a generated config with allow_generated",
                [
                    native_config_error(
                        "taplo",
                        "gen/taplo.toml",
                        False,
                        [],
                        True,
                    ),
                    native_config_error(
                        "ruff",
                        "gen/ruff.toml",
                        False,
                        [struct(is_source = False, path = "gen/extra.toml")],
                        True,
                    ),
                ],
                ["", ""],
            ),
            expect_equal(
                "native_config_error accepts generated data with allow_generated",
                native_config_error(
                    "vale",
                    ".vale.ini",
                    True,
                    [struct(is_source = False, path = "gen/Cotton.yml")],
                    True,
                ),
                "",
            ),
            expect_equal(
                "native_config_error keeps suffix checks for generated configs",
                native_config_error("vale", ".vale.json", False, [], True),
                "native_config (vale): src must end in '.ini', got .vale.json",
            ),
            expect_equal(
                "native_config_error rejects a mis-suffixed config",
                native_config_error("vale", ".vale.json", True, []),
                "native_config (vale): src must end in '.ini', got .vale.json",
            ),
            expect_equal(
                "native_config_error rejects a generated data member",
                native_config_error(
                    "vale",
                    ".vale.ini",
                    True,
                    [struct(is_source = False, path = "gen/Cotton.yml")],
                ),
                "native_config (vale): data must be checked-in source " +
                "files, got generated gen/Cotton.yml",
            ),
            expect_equal(
                "collect_native_configs resolves hints in stage-tool order",
                collect_native_configs(
                    _HINTS,
                    ["taplo", "buildifier"],
                    "//q:t",
                ),
                {"buildifier": _HINTS[1], "taplo": _HINTS[2]},
            ),
            expect_equal(
                "collect_native_configs omits stage tools without a hint",
                collect_native_configs(_HINTS, ["vale", "taplo"], "//q:t"),
                {"taplo": _HINTS[2]},
            ),
            expect_equal(
                "collect_native_configs ignores hints outside the stages",
                collect_native_configs(_HINTS, ["taplo"], "//q:t"),
                {"taplo": _HINTS[2]},
            ),
            expect_equal(
                "bound_config_targets falls back to the policy and lets a hint win",
                _names(bound_config_targets(
                    struct(aspect_hints = [_TARGET_RUFF]),
                    {"ruff": _TARGET_POLICY_RUFF, "dart": _TARGET_DART},
                    "//q:t",
                )),
                {"ruff": "hint", "dart": "dart"},
            ),
            expect_equal(
                "bound_config_targets uses the policy for a rule without aspect_hints",
                _names(bound_config_targets(struct(), {"ruff": _TARGET_POLICY_RUFF}, "//q:t")),
                {"ruff": "policy"},
            ),
            expect_equal(
                "collect_native_configs uses a default when the target has no hint",
                collect_native_configs([], ["ruff"], "//q:t", {"ruff": _HINTS[3]}),
                {"ruff": _HINTS[3]},
            ),
            expect_equal(
                "collect_native_configs lets a hint win over a default",
                collect_native_configs([_HINTS[3]], ["ruff"], "//q:t", {"ruff": struct(tool_id = "ruff", other = True)}),
                {"ruff": _HINTS[3]},
            ),
            expect_equal(
                "collect_native_configs resolves helm through a kubeconform default",
                collect_native_configs([], ["helm"], "//q:t", {"kubeconform": struct(tool_id = "kubeconform")}),
                {"helm": struct(tool_id = "kubeconform")},
            ),
            expect_equal(
                "collect_native_configs is empty without hints",
                collect_native_configs([], ["taplo"], "//q:t"),
                {},
            ),
            expect_equal(
                "missing_required_config_error uses the tool hint for kubeconform",
                missing_required_config_error("kubeconform", "lint", {}, "//q:t"),
                "kubeconform needs a config. Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes/rules:defs.bzl and list it in your dx_generated_policy configs or add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint.",
            ),
            expect_equal(
                "collect_native_configs resolves helm through the kubeconform hint",
                collect_native_configs([struct(tool_id = "kubeconform")], ["helm", "kubeconform"], "//q:t"),
                {"helm": struct(tool_id = "kubeconform"), "kubeconform": struct(tool_id = "kubeconform")},
            ),
            expect_equal(
                "collect_native_configs gives helm nothing without the kubeconform hint",
                collect_native_configs([], ["helm"], "//q:t"),
                {},
            ),
            expect_equal(
                "missing_required_config_error names kubernetes_schemas for helm",
                missing_required_config_error("helm", "lint", {}, "//q:t"),
                "helm needs a config. Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes/rules:defs.bzl and list it in your dx_generated_policy configs or add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint. helm validates its rendered manifests with the same schemas.",
            ),
            expect_equal(
                "native_config rejects helm as a tool id",
                native_config_error("helm", "helm.json", True, []).startswith("native_config: unknown tool 'helm'"),
                True,
            ),
            expect_equal(
                "every required-config sample src passes native_config_error",
                [
                    tool
                    for tool in CONFIG_REQUIRED_TOOLS
                    if tool not in CONFIG_TOOL_ALIASES and native_config_error(tool, CONFIG_REQUIRED_TOOLS[tool].src, True, []) != ""
                ],
                [],
            ),
            expect_equal(
                "prettier config names are accepted or rejected by suffix",
                [
                    native_config_extension("prettier"),
                    native_config_error("prettier", ".prettierrc", True, []),
                    native_config_error("prettier", "cfg/.prettierrc.json", True, []),
                    native_config_error("prettier", "cfg/prettier.config.mjs", True, []),
                    native_config_error("prettier", "cfg/prettier.config.ts", True, []) != "",
                ],
                [".prettierrc", "", "", "", True],
            ),
            expect_equal(
                "every tool the runner refuses without a config is required here",
                sorted([
                    tool
                    for tool in _RUNNER_CONFIG_REQUIRED_TOOLS
                    if missing_required_config_error(tool, "lint", {}, "//q:t") == ""
                ]),
                [],
            ),
            expect_equal(
                "the runner's config-required tools all carry a config transport",
                [
                    native_config_extension(tool)
                    for tool in _RUNNER_CONFIG_REQUIRED_TOOLS
                ],
                [],
            ),
            expect_equal(
                "missing_required_config_error leaves tools with a tier base and bound tools alone",
                [
                    missing_required_config_error(tool, "lint", {}, "//q:t")
                    for tool in ["biome", "buildifier", "checkstyle", "eslint", "graphql_eslint", "ruff", "ty", "vale"]
                ] + [missing_required_config_error("kubeconform", "lint", {"kubeconform": _HINTS[0]}, "//q:t")],
                ["", "", "", "", "", "", "", "", ""],
            ),
        ],
    )
