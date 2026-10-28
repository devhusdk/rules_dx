"""Unit tests for typed native-config validation."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(
    ":native_config.bzl",
    "CONFIG_REQUIRED_TOOLS",
    "CONFIG_TOOL_ALIASES",
    "collect_native_configs",
    "missing_required_config_error",
    "native_config_error",
    "native_config_extension",
    "native_config_extensions",
    "native_config_scratch_rel",
)

_HINTS = [
    struct(tool_id = "biome"),
    struct(tool_id = "buildifier"),
    struct(tool_id = "taplo"),
    struct(tool_id = "ruff"),
    struct(tool_id = "eslint"),
    struct(tool_id = "scalafmt"),
]

# Each entry is pinned to its runner refusal by the file_checks below.
_RUNNER_CONFIG_REQUIRED_TOOLS = [
    "checkstyle",
    "eslint",
    "graphql_eslint",
    "scalafix",
    "sqlfluff",
    "vale",
]

_RUNNER_CONFIG_REFUSALS = "\n".join(
    ['"' + tool + ' requires a config"' for tool in _RUNNER_CONFIG_REQUIRED_TOOLS],
)

def native_config_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
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
                "ansible_lint, biome, buf, buildifier, cfn_lint, checkmake, checkstyle, clang_format, clang_tidy, cmakelint, cppcheck, credo, csharpier, dart, djlint, elvis, eslint, fsharplint, gersemi, gplint, graphql_eslint, hadolint, kubeconform, latexindent, mbake, phpcs, phpstan, pmd, prettier, psscriptanalyzer, qmlformat, qmllint, redocly, regal, remark, rubocop, ruff, rustfmt, scalafix, scalafmt, shellcheck, sqlfluff, standardrb, staticcheck, statix, stylelint, stylua, taplo, tflint, vale, yamlfmt, yamllint",
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
                "collect_native_configs is empty without hints",
                collect_native_configs([], ["taplo"], "//q:t"),
                {},
            ),
            expect_equal(
                "missing_required_config_error names an unbound buildifier lint stage",
                missing_required_config_error("buildifier", "lint", {}, "//q:t"),
                "//q:t: Buildifier needs a config. Declare buildifier_config(name = \"buildifier_config\", src = \".buildifier.json\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":buildifier_config\"], or tag the target no-lint.",
            ),
            expect_equal(
                "missing_required_config_error uses the tool hint for kubeconform",
                missing_required_config_error("kubeconform", "lint", {}, "//q:t"),
                "//q:t: kubeconform needs a config. Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes/rules:defs.bzl and add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint.",
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
                "//q:t: helm needs a config. Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes/rules:defs.bzl and add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint. helm validates its rendered manifests with the same schemas.",
            ),
            expect_equal(
                "native_config rejects helm as a tool id",
                native_config_error("helm", "helm.json", True, []).startswith("native_config: unknown tool 'helm'"),
                True,
            ),
            expect_equal(
                "missing_required_config_error names an unbound checkstyle lint stage",
                missing_required_config_error("checkstyle", "lint", {}, "//q:t"),
                "//q:t: Checkstyle needs a config. Declare checkstyle_config(name = \"checkstyle_config\", src = \"checkstyle.xml\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":checkstyle_config\"], or tag the target no-lint.",
            ),
            expect_equal(
                "missing_required_config_error names an unbound vale lint stage",
                missing_required_config_error("vale", "lint", {}, "//q:t"),
                "//q:t: Vale needs a config. Declare vale_config(name = \"vale_config\", src = \".vale.ini\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":vale_config\"], or tag the target no-lint.",
            ),
            expect_equal(
                "missing_required_config_error names an unbound ruff format stage",
                missing_required_config_error("ruff", "format", {}, "//q:t"),
                "//q:t: Ruff needs a config. Declare ruff_config(name = \"ruff_config\", src = \"ruff.toml\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":ruff_config\"], or tag the target no-format.",
            ),
            expect_equal(
                "missing_required_config_error names an unbound ruff lint stage",
                missing_required_config_error("ruff", "lint", {}, "//q:t"),
                "//q:t: Ruff needs a config. Declare ruff_config(name = \"ruff_config\", src = \"ruff.toml\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":ruff_config\"], or tag the target no-lint.",
            ),
            expect_equal(
                "missing_required_config_error names an unbound biome format stage",
                missing_required_config_error("biome", "format", {}, "//q:t"),
                "//q:t: Biome needs a config. Declare biome_config(name = \"biome_config\", src = \"biome.json\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":biome_config\"], or tag the target no-format.",
            ),
            expect_equal(
                "missing_required_config_error names an unbound biome lint stage",
                missing_required_config_error("biome", "lint", {}, "//q:t"),
                "//q:t: Biome needs a config. Declare biome_config(name = \"biome_config\", src = \"biome.json\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":biome_config\"], or tag the target no-lint.",
            ),
            expect_equal(
                "missing_required_config_error names an unbound eslint lint stage",
                missing_required_config_error("eslint", "lint", {}, "//q:t"),
                "//q:t: ESLint needs a config. Declare eslint_config(name = \"eslint_config\", src = \"eslint.config.mjs\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":eslint_config\"], or tag the target no-lint.",
            ),
            expect_equal(
                "missing_required_config_error names an unbound scalafmt format stage",
                missing_required_config_error("scalafmt", "format", {}, "//q:t"),
                "//q:t: Scalafmt needs a config. Declare scalafmt_config(name = \"scalafmt_config\", src = \".scalafmt.conf\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":scalafmt_config\"], or tag the target no-format.",
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
                "missing_required_config_error names an unbound graphql_eslint lint stage",
                missing_required_config_error("graphql_eslint", "lint", {}, "//q:t"),
                "//q:t: graphql-eslint needs a config. Declare graphql_eslint_config(name = \"graphql_eslint_config\", src = \"graphql_lint.json\") from @rules_dx//quality:native_config.bzl and add aspect_hints = [\":graphql_eslint_config\"], or tag the target no-lint.",
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
                [".xml", ".js", ".json", ".conf", ".sqlfluff", ".ini"],
            ),
            expect_equal(
                "missing_required_config_error accepts bound and default-only tools",
                [
                    missing_required_config_error("buildifier", "lint", {"buildifier": _HINTS[1]}, "//q:t"),
                    missing_required_config_error("ruff", "lint", {"ruff": _HINTS[3]}, "//q:t"),
                    missing_required_config_error("biome", "lint", {"biome": _HINTS[0]}, "//q:t"),
                    missing_required_config_error("eslint", "lint", {"eslint": _HINTS[4]}, "//q:t"),
                    missing_required_config_error("scalafmt", "lint", {"scalafmt": _HINTS[5]}, "//q:t"),
                    missing_required_config_error("taplo", "lint", {}, "//q:t"),
                    missing_required_config_error("pmd", "lint", {}, "//q:t"),
                    missing_required_config_error("prettier", "format", {}, "//q:t"),
                    missing_required_config_error("remark", "format", {}, "//q:t"),
                ],
                ["", "", "", "", "", "", "", "", ""],
            ),
        ],
        file_checks = {
            "//quality/runner:src/real/check.rs": _RUNNER_CONFIG_REFUSALS,
        },
    )
