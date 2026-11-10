"""Native-config providers and helpers shared by the quality rules."""

load("//core/quality:tools.bzl", "TOOLS")

visibility(["//core/quality/...", "//javascript/private/quality"])

DxNativeConfigInfo = provider(
    doc = "One tool-owned native config file plus its checked-in closure.",
    fields = {
        "closure": "depset[File]: config plus every data file the tool reaches.",
        "config": "File: the tool-owned config file passed to the adapter.",
        "tool_id": "str: stable built-in tool identifier from REAL_ADAPTERS.",
    },
)

CONFIG_REQUIRED_TOOLS = {
    "kubeconform": struct(
        name = "kubeconform",
        src = "kubernetes.kubeconform.json",
        hint = "Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes:defs.bzl and add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint.",
    ),
    "helm": struct(
        name = "helm",
        src = "kubernetes.kubeconform.json",
        hint = "Declare kubernetes_schemas(name = \"kubernetes_schemas\", kubernetes_version = \"1.32.0\", schemas = [...]) from @rules_dx//kubernetes:defs.bzl and add aspect_hints = [\":kubernetes_schemas\"], or tag the target no-lint. helm validates its rendered manifests with the same schemas.",
    ),
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

_TIER_BASE_DIRS = {tool.id: tool.config.tier_dir or tool.id for tool in TOOLS if tool.config.tier_base}

def tier_base_dirs():
    """Returns tool ID to its directory under core/quality/tiers for every tool whose tier base is bound."""
    return _TIER_BASE_DIRS

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
    return info.name + " needs a config. " + info.hint
