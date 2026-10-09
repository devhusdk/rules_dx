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
    "biome": "Biome",
    "buildifier": "Buildifier",
    "checkstyle": "Checkstyle",
    "eslint": "ESLint",
    "ruff": "Ruff",
    "vale": "Vale",
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
    "djlint": [".toml"],
    "eslint": [".js", ".mjs", ".cjs"],
    "fsharplint": [".json"],
    "qmlformat": [".ini"],
    "qmllint": [".ini"],
    "ruff": [".toml"],
    "rustfmt": [".toml"],
    "scalafix": [".conf"],
    "scalafmt": [".conf"],
    "staticcheck": [".conf"],
    "stylelint": [".json"],
    "taplo": [".toml"],
    "vale": [".ini"],
    "yamllint": [".yaml"],
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

def missing_required_config_error(tool, configs_by_tool, what):
    """Returns the error for a required tool with no bound config, or ""."""
    if tool not in CONFIG_REQUIRED_TOOLS or tool in configs_by_tool:
        return ""
    return ("real_aspect (" + what + "): applicable " + CONFIG_REQUIRED_TOOLS[tool] +
            " requires declared config; supply and bind native policy via " +
            "aspect_hints (no usable upstream default)")

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
        attrs = {
            "allow_generated": attr.bool(
                default = False,
            ),
            "data": attr.label_list(
                allow_files = True,
                default = [],
            ),
            "src": attr.label(
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

buf_config = _make_native_config_rule("buf")

qmlformat_config = _make_native_config_rule("qmlformat")

qmllint_config = _make_native_config_rule("qmllint")

stylelint_config = _make_native_config_rule("stylelint")

djlint_config = _make_native_config_rule("djlint")

yamllint_config = _make_native_config_rule("yamllint")
