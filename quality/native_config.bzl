"""Typed native-configuration targets for adapters."""

DxNativeConfigInfo = provider(
    doc = "One tool-owned native config file plus its checked-in closure.",
    fields = {
        "closure": "depset[File]: config plus every data file the tool reaches.",
        "config": "File: the tool-owned config file passed to the adapter.",
        "tool_id": "str: stable built-in tool identifier from REAL_ADAPTERS.",
    },
)

CONFIG_REQUIRED_TOOLS = {
    "biome": struct(name = "Biome", src = "biome.json"),
    "buildifier": struct(name = "Buildifier", src = ".buildifier.json"),
    "checkstyle": struct(name = "Checkstyle", src = "checkstyle.xml"),
    "eslint": struct(name = "ESLint", src = "eslint.config.mjs"),
    "graphql_eslint": struct(name = "graphql-eslint", src = "graphql_lint.json"),
    "ruff": struct(name = "Ruff", src = "ruff.toml"),
    "scalafix": struct(name = "Scalafix", src = ".scalafix.conf"),
    "scalafmt": struct(name = "Scalafmt", src = ".scalafmt.conf"),
    "vale": struct(name = "Vale", src = ".vale.ini"),
}

_NATIVE_CONFIG_SUFFIXES = {
    "biome": [".json"],
    "buf": [".yaml"],
    "buildifier": [".json"],
    "checkstyle": [".xml"],
    "clang_format": [".clang-format"],
    "clang_tidy": [".clang-tidy"],
    "cppcheck": [".txt"],
    "csharpier": [".yaml"],
    "djlint": ["djlint.toml", ".djlintrc"],
    "eslint": [".js", ".mjs", ".cjs"],
    "fsharplint": [".json"],
    "graphql_eslint": [".json"],
    "hadolint": [".yaml", ".yml"],
    "pmd": [".xml"],
    "prettier": [".prettierrc", ".json", ".json5", ".yaml", ".yml", ".toml", ".mjs", ".cjs", ".js"],
    "psscriptanalyzer": [".psd1"],
    "qmlformat": [".qmlformat.ini"],
    "qmllint": [".qmllint.ini"],
    "redocly": [".yaml", ".yml"],
    "remark": [".json"],
    "rubocop": [".yml", ".yaml"],
    "ruff": [".toml"],
    "rustfmt": [".toml"],
    "scalafix": [".conf"],
    "scalafmt": [".conf"],
    "shellcheck": [".shellcheckrc", "shellcheckrc"],
    "standardrb": [".yml", ".yaml"],
    "staticcheck": [".conf"],
    "stylelint": [".json"],
    "taplo": [".toml"],
    "vale": [".ini"],
    "yamlfmt": [".yaml", ".yml", ".yamlfmt"],
    "yamllint": [".yaml", ".yml", ".yamllint"],
}

def native_config_extensions(tool_id):
    """Returns the accepted config suffixes for a real tool ID."""
    if tool_id not in _NATIVE_CONFIG_SUFFIXES:
        fail("native_config: unknown tool '" + tool_id +
             "': want one of " + ", ".join(sorted(_NATIVE_CONFIG_SUFFIXES.keys())))
    return _NATIVE_CONFIG_SUFFIXES[tool_id]

def native_config_extension(tool_id):
    """Returns the canonical config suffix for a real tool ID."""
    return native_config_extensions(tool_id)[0]

def _native_config_suffix_error(tool_id, config_path, wants):
    if len(wants) == 1:
        return ("native_config (" + tool_id + "): src must end in '" + wants[0] +
                "', got " + config_path)
    quoted = ["'" + want + "'" for want in wants]
    return ("native_config (" + tool_id + "): src must end in one of " +
            ", ".join(quoted) + ", got " + config_path)

def native_config_error(tool_id, config_path, config_is_source, data, allow_generated = False):
    """Returns the validation error for a native config, or "" when valid."""
    if tool_id not in _NATIVE_CONFIG_SUFFIXES:
        return ("native_config: unknown tool '" + tool_id + "': want one of " +
                ", ".join(sorted(_NATIVE_CONFIG_SUFFIXES.keys())))
    if config_path == "":
        return "native_config (" + tool_id + "): src is required"
    if not config_is_source and not allow_generated:
        return ("native_config (" + tool_id + "): src must be a checked-in " +
                "source file, got generated " + config_path)
    wants = _NATIVE_CONFIG_SUFFIXES[tool_id]
    accepted = [want for want in wants if config_path.endswith(want)]
    if len(accepted) == 0:
        return _native_config_suffix_error(tool_id, config_path, wants)
    for entry in data:
        if not entry.is_source and not allow_generated:
            return ("native_config (" + tool_id + "): data must be " +
                    "checked-in source files, got generated " + entry.path)
    return ""

def collect_native_configs(hints, stage_tools, what):
    """Resolves aspect hints to the configs for a pipeline's stage tools."""
    by_tool = {}
    for hint in hints:
        if hint.tool_id in by_tool:
            fail("native_config (" + what + "): duplicate aspect_hints " +
                 "for tool '" + hint.tool_id + "'")
        by_tool[hint.tool_id] = hint
    return {tool: by_tool[tool] for tool in stage_tools if tool in by_tool}

def missing_required_config_error(tool, capability, configs_by_tool, what):
    """Returns the error for a required tool with no bound config, or ""."""
    if tool not in CONFIG_REQUIRED_TOOLS or tool in configs_by_tool:
        return ""
    info = CONFIG_REQUIRED_TOOLS[tool]
    return (what + ": " + info.name + " needs a config. Declare " + tool + "_config(name = \"" +
            tool + "_config\", src = \"" + info.src + "\") from @rules_dx//quality:native_config.bzl" +
            " and add aspect_hints = [\":" + tool + "_config\"], or tag the target no-" + capability + ".")

def _native_config_impl(ctx):
    tool_id = ctx.attr._tool_id
    config = ctx.file.src
    config_path = config.path if config else ""
    config_is_source = config.is_source if config else False
    data = [struct(path = f.path, is_source = f.is_source) for f in ctx.files.data]
    err = native_config_error(tool_id, config_path, config_is_source, data, ctx.attr.allow_generated)
    if err != "":
        fail(err + " (in " + str(ctx.label) + ")")
    closure = depset([config] + ctx.files.data)
    return [
        DefaultInfo(files = closure),
        DxNativeConfigInfo(
            closure = closure,
            config = config,
            tool_id = tool_id,
        ),
    ]

def _make_native_config_rule(tool_id):
    return rule(
        implementation = _native_config_impl,
        doc = "Native config file for " + tool_id + ".",
        attrs = {
            "allow_generated": attr.bool(
                doc = "Accepts generated config and data files, not only checked-in sources.",
                default = False,
            ),
            "data": attr.label_list(
                doc = "Extra files the tool reads alongside its config.",
                allow_files = True,
                default = [],
            ),
            "src": attr.label(
                doc = "Config file the tool reads, normally a checked-in source.",
                allow_single_file = True,
                mandatory = True,
            ),
            "_tool_id": attr.string(
                default = tool_id,
            ),
        },
    )

buildifier_config = _make_native_config_rule("buildifier")

taplo_config = _make_native_config_rule("taplo")

vale_config = _make_native_config_rule("vale")

rustfmt_config = _make_native_config_rule("rustfmt")

ruff_config = _make_native_config_rule("ruff")

biome_config = _make_native_config_rule("biome")

eslint_config = _make_native_config_rule("eslint")

scalafmt_config = _make_native_config_rule("scalafmt")

scalafix_config = _make_native_config_rule("scalafix")

csharpier_config = _make_native_config_rule("csharpier")

clang_format_config = _make_native_config_rule("clang_format")

clang_tidy_config = _make_native_config_rule("clang_tidy")

cppcheck_config = _make_native_config_rule("cppcheck")

staticcheck_config = _make_native_config_rule("staticcheck")

fsharplint_config = _make_native_config_rule("fsharplint")

checkstyle_config = _make_native_config_rule("checkstyle")

pmd_config = _make_native_config_rule("pmd")

buf_config = _make_native_config_rule("buf")

qmlformat_config = _make_native_config_rule("qmlformat")

qmllint_config = _make_native_config_rule("qmllint")

prettier_config = _make_native_config_rule("prettier")

stylelint_config = _make_native_config_rule("stylelint")

graphql_eslint_config = _make_native_config_rule("graphql_eslint")

redocly_config = _make_native_config_rule("redocly")
remark_config = _make_native_config_rule("remark")

psscriptanalyzer_config = _make_native_config_rule("psscriptanalyzer")

djlint_config = _make_native_config_rule("djlint")

yamlfmt_config = _make_native_config_rule("yamlfmt")

shellcheck_config = _make_native_config_rule("shellcheck")

yamllint_config = _make_native_config_rule("yamllint")

hadolint_config = _make_native_config_rule("hadolint")

rubocop_config = _make_native_config_rule("rubocop")

standardrb_config = _make_native_config_rule("standardrb")
