"""Typed native-configuration targets for adapters."""

load("//dart/rules:providers.bzl", "DartInfo")
load(":tools.bzl", "TOOLS")

DxNativeConfigInfo = provider(
    doc = "One tool-owned native config file plus its checked-in closure.",
    fields = {
        "closure": "depset[File]: config plus every data file the tool reaches.",
        "config": "File: the tool-owned config file passed to the adapter.",
        "tool_id": "str: stable built-in tool identifier from REAL_ADAPTERS.",
    },
)

DxDartConfigInfo = provider(
    doc = "Dart packages a dart_config pulls in for the analyzer.",
    fields = {
        "packages": "depset[DartPackageInfo]: packages of the config deps.",
        "resources": "depset[File]: non-Dart lib files of those packages.",
        "srcs": "depset[File]: Dart sources of those packages.",
    },
)

CONFIG_REQUIRED_TOOLS = {
    "biome": struct(name = "Biome", src = "biome.json", capabilities = ["lint"]),
    "buildifier": struct(name = "Buildifier", src = ".buildifier.json"),
    "checkstyle": struct(name = "Checkstyle", src = "checkstyle.xml"),
    "eslint": struct(name = "ESLint", src = "eslint.config.mjs"),
    "graphql_eslint": struct(name = "graphql-eslint", src = "graphql_lint.json"),
    "kubeconform": struct(
        name = "kubeconform",
        src = "kubernetes.kubeconform.json",
        hint = "Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes/rules:defs.bzl and list it in your workspace_policy configs or add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint.",
    ),
    "helm": struct(
        name = "helm",
        src = "kubernetes.kubeconform.json",
        hint = "Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes/rules:defs.bzl and list it in your workspace_policy configs or add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint. helm validates its rendered manifests with the same schemas.",
    ),
    "ruff": struct(name = "Ruff", src = "ruff.toml"),
    "ty": struct(name = "ty", src = "ty.toml"),
    "sqlfluff": struct(
        name = "sqlfluff",
        src = ".sqlfluff",
        hint = "Declare sqlfluff_config(name = \"sqlfluff_config\", src = \".sqlfluff\") from @rules_dx//quality:native_config.bzl and list it in your workspace_policy configs or add aspect_hints = [\":sqlfluff_config\"], or tag the target no-lint. The file must set `dialect = ...` under [sqlfluff] using sqlfluff's names (postgres, sparksql, tsql and oracle rather than sql_library's postgresql, spark, transactsql and plsql).",
    ),
    "vale": struct(name = "Vale", src = ".vale.ini"),
}

CONFIG_TOOL_ALIASES = {"helm": "kubeconform"}

_NATIVE_CONFIG_SUFFIXES = {tool.id: tool.config.suffixes for tool in TOOLS if tool.config.suffixes}

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

_STAGED_CONFIG_FILES = {tool.id: tool.config.files[0] for tool in TOOLS if tool.config.stage_native and tool.config.files}

_TIER_ACTIVE_DIRS = {tool.id: tool.config.tier_dir or tool.id for tool in TOOLS if tool.config.tier_active}

def tier_active_dirs():
    """Returns tool ID to its directory under quality/tiers for every tier-active tool."""
    return _TIER_ACTIVE_DIRS

def staged_config_rel(tool_id, config_rel):
    """Returns where a bound config is staged: files[0] next to it for stage_native tools, else unchanged."""
    name = _STAGED_CONFIG_FILES.get(CONFIG_TOOL_ALIASES.get(tool_id, tool_id))
    if name == None:
        return config_rel
    directory, slash, _ = config_rel.rpartition("/")
    return directory + slash + name

def native_config_scratch_rel(short_path):
    """Maps a short_path to a scratch-relative path ('../repo/p' -> '_external/repo/p'), or None if it still escapes."""
    rel = short_path
    if rel.startswith("../"):
        rel = "_external/" + rel[3:]
    if ".." in rel.split("/"):
        return None
    return rel

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

def collect_native_configs(hints, stage_tools, what, defaults = {}):
    """Resolves aspect hints, then the policy defaults, to the configs for a pipeline's stage tools."""
    by_tool = {}
    for hint in hints:
        if hint.tool_id in by_tool:
            fail("native_config (" + what + "): duplicate aspect_hints " +
                 "for tool '" + hint.tool_id + "'")
        by_tool[hint.tool_id] = hint
    resolved = {}
    for tool in stage_tools:
        source = CONFIG_TOOL_ALIASES.get(tool, tool)
        if source in by_tool:
            resolved[tool] = by_tool[source]
        elif source in defaults:
            resolved[tool] = defaults[source]
    return resolved

def bound_config_targets(rule_attr, policy_configs, what):
    """Returns tool ID to the config Target a rule uses: its aspect_hints first, then the policy."""
    bound = {}
    for hint_target in getattr(rule_attr, "aspect_hints", []):
        if DxNativeConfigInfo not in hint_target:
            continue
        tool_id = hint_target[DxNativeConfigInfo].tool_id
        if tool_id in bound:
            fail("native_config (" + what + "): duplicate aspect_hints " +
                 "for tool '" + tool_id + "'")
        bound[tool_id] = hint_target
    for tool_id, target in policy_configs.items():
        if tool_id not in bound:
            bound[tool_id] = target
    return bound

def missing_required_config_error(tool, capability, configs_by_tool, what):
    """Returns the error for a required tool with no bound config, or ""."""
    if tool not in CONFIG_REQUIRED_TOOLS or tool in configs_by_tool:
        return ""
    info = CONFIG_REQUIRED_TOOLS[tool]
    if hasattr(info, "capabilities") and capability not in info.capabilities:
        return ""
    if hasattr(info, "hint"):
        return info.name + " needs a config. " + info.hint
    return (info.name + " needs a config. Declare " + tool + "_config(name = \"" +
            tool + "_config\", src = \"" + info.src + "\") from @rules_dx//quality:native_config.bzl" +
            " and list it in your workspace_policy configs or add aspect_hints = [\":" + tool + "_config\"], or tag the target no-" + capability + ".")

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

stylua_config = _make_native_config_rule("stylua")

selene_config = _make_native_config_rule("selene")

taplo_config = _make_native_config_rule("taplo")

latexindent_config = _make_native_config_rule("latexindent")

vale_config = _make_native_config_rule("vale")

rustfmt_config = _make_native_config_rule("rustfmt")

ruff_config = _make_native_config_rule("ruff")

ty_config = _make_native_config_rule("ty")

biome_config = _make_native_config_rule("biome")

eslint_config = _make_native_config_rule("eslint")

scalafmt_config = _make_native_config_rule("scalafmt")

scalafix_config = _make_native_config_rule("scalafix")

sqlfluff_config = _make_native_config_rule("sqlfluff")

csharpier_config = _make_native_config_rule("csharpier")

def _dart_config_impl(ctx):
    result = _native_config_impl(ctx)
    infos = [dep[DartInfo] for dep in ctx.attr.deps]
    return result + [DxDartConfigInfo(
        packages = depset(transitive = [i.transitive_packages for i in infos]),
        resources = depset(transitive = [i.transitive_resources for i in infos]),
        srcs = depset(transitive = [i.transitive_srcs for i in infos]),
    )]

dart_config = rule(
    implementation = _dart_config_impl,
    doc = "Native config file for dart, with the packages its include: lines name.",
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
        "deps": attr.label_list(
            doc = "Dart packages the config includes, for example @my_pub_deps//:lints.",
            providers = [DartInfo],
            default = [],
        ),
        "src": attr.label(
            doc = "Config file the tool reads, normally a checked-in source.",
            allow_single_file = True,
            mandatory = True,
        ),
        "_tool_id": attr.string(
            default = "dart",
        ),
    },
)

clang_format_config = _make_native_config_rule("clang_format")

clang_tidy_config = _make_native_config_rule("clang_tidy")

cppcheck_config = _make_native_config_rule("cppcheck")

roslyn_config = _make_native_config_rule("roslyn")

staticcheck_config = _make_native_config_rule("staticcheck")

fsharplint_config = _make_native_config_rule("fsharplint")

checkstyle_config = _make_native_config_rule("checkstyle")

pmd_config = _make_native_config_rule("pmd")

buf_config = _make_native_config_rule("buf")

qmlformat_config = _make_native_config_rule("qmlformat")

qmllint_config = _make_native_config_rule("qmllint")

prettier_config = _make_native_config_rule("prettier")

stylelint_config = _make_native_config_rule("stylelint")

gplint_config = _make_native_config_rule("gplint")

graphql_eslint_config = _make_native_config_rule("graphql_eslint")

redocly_config = _make_native_config_rule("redocly")

markdown_check_config = _make_native_config_rule("markdown_check")
remark_config = _make_native_config_rule("remark")

psscriptanalyzer_config = _make_native_config_rule("psscriptanalyzer")

djlint_config = _make_native_config_rule("djlint")

yamlfmt_config = _make_native_config_rule("yamlfmt")

shellcheck_config = _make_native_config_rule("shellcheck")

yamllint_config = _make_native_config_rule("yamllint")

ansible_lint_config = _make_native_config_rule("ansible_lint")

bicep_config = _make_native_config_rule("bicep")

cfn_lint_config = _make_native_config_rule("cfn_lint")

hadolint_config = _make_native_config_rule("hadolint")
hlint_config = _make_native_config_rule("hlint")
regal_config = _make_native_config_rule("regal")
checkmake_config = _make_native_config_rule("checkmake")
gersemi_config = _make_native_config_rule("gersemi")
cmakelint_config = _make_native_config_rule("cmakelint")
ffmt_config = _make_native_config_rule("ffmt")
mbake_config = _make_native_config_rule("mbake")
tflint_config = _make_native_config_rule("tflint")
trivy_config_config = _make_native_config_rule("trivy_config")
zizmor_config = _make_native_config_rule("zizmor")
fortitude_config = _make_native_config_rule("fortitude")
swift_format_config = _make_native_config_rule("swift_format")
swift_lint_config = _make_native_config_rule("swift_lint")
credo_config = _make_native_config_rule("credo")
detekt_config = _make_native_config_rule("detekt")

elvis_config = _make_native_config_rule("elvis")

luals_config = _make_native_config_rule("luals")

perlcritic_config = _make_native_config_rule("perlcritic")

phpcs_config = _make_native_config_rule("phpcs")

phpstan_config = _make_native_config_rule("phpstan")

rubocop_config = _make_native_config_rule("rubocop")


