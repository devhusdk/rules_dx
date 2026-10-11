"""Explicit tool support records."""

load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")

SUPPORT_SCHEMA_VERSION = 1

TEXT_KEEP_SORTED_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["text"],
    "context": "owned",
    "execution": "direct",
    "family": "text",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "keep_sorted",
    "transport": "batched",
}

SHELL_SHFMT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["shell"],
    "context": "owned",
    "execution": "direct",
    "family": "shell",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "shfmt",
    "transport": "single",
}

SHELL_SHELLCHECK_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["shell"],
    "context": "owned",
    "execution": "direct",
    "family": "shell",
    "fix": "check_only",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "qualified",
    "tool": "shellcheck",
    "transport": "single",
}

ALL_TOOL_SUPPORT = [
    TEXT_KEEP_SORTED_SUPPORT,
    SHELL_SHFMT_SUPPORT,
    SHELL_SHELLCHECK_SUPPORT,
]

def _is_canonical_token(text):
    """Reports whether a token uses the canonical alphabet."""
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def support_record_error(record):
    """Validates one support record against the adapter taxonomy."""
    if SUPPORT_SCHEMA_VERSION != 1:
        return "support: unsupported schema v" + str(SUPPORT_SCHEMA_VERSION) + " (want v1)"
    for key in ["artifact", "capability", "classes", "context", "execution", "family", "fix", "platforms", "state", "tool", "transport"]:
        if key not in record:
            return "support: record is missing field '" + key + "'"
    tool = record["tool"]
    capability = record["capability"]
    family = record["family"]
    if not _is_canonical_token(tool):
        return "support: non-canonical tool '" + str(tool) + "'"
    if capability not in ["audit", "format", "lint", "typecheck"]:
        return "support: unknown capability '" + str(capability) + "' for tool '" + tool + "'"
    if not _is_canonical_token(family):
        return "support: non-canonical family '" + str(family) + "'"
    if tool not in REAL_ADAPTERS:
        return "support: unknown tool '" + tool + "': not in the real adapter registry"
    if capability not in REAL_ADAPTERS[tool]:
        return "support: tool '" + tool + "' has no '" + capability + "' capability"
    classes = record["classes"]
    if type(classes) != "list" or len(classes) == 0:
        return "support: tool '" + tool + "' needs a non-empty class list"
    for class_id in classes:
        if not _is_canonical_token(class_id):
            return "support: non-canonical class '" + str(class_id) + "' for tool '" + tool + "'"
        if class_id not in REAL_CLASS_TO_FAMILY:
            return "support: tool '" + tool + "' names unclassified class '" + class_id + "'"
        if REAL_CLASS_TO_FAMILY[class_id] != family:
            return "support: class '" + class_id + "' belongs to family '" + REAL_CLASS_TO_FAMILY[class_id] + "', not '" + family + "'"
        if class_id not in REAL_ADAPTERS[tool][capability]:
            return "support: class '" + class_id + "' is outside '" + tool + " " + capability + "' adapter support"
    if record["execution"] not in ["delegated", "direct"]:
        return "support: tool '" + tool + "' needs execution 'direct' or 'delegated', got '" + str(record["execution"]) + "'"
    if record["fix"] not in ["check_only", "supported"]:
        return "support: tool '" + tool + "' needs fix 'supported' or 'check_only', got '" + str(record["fix"]) + "'"
    if record["context"] not in ["owned"]:
        return "support: tool '" + tool + "' needs context 'owned', got '" + str(record["context"]) + "'"
    platforms = record["platforms"]
    if type(platforms) != "list" or len(platforms) == 0:
        return "support: tool '" + tool + "' needs a non-empty platform list"
    for platform in platforms:
        if not _is_canonical_token(platform):
            return "support: non-canonical platform '" + str(platform) + "' for tool '" + tool + "'"
    if record["state"] not in ["known", "qualified", "supported"]:
        return "support: tool '" + tool + "' needs state 'known', 'supported' or 'qualified', got '" + str(record["state"]) + "'"
    if record["artifact"] not in ["missing", "pinned"]:
        return "support: tool '" + tool + "' needs artifact 'missing' or 'pinned', got '" + str(record["artifact"]) + "'"
    if record["transport"] not in ["batched", "single"]:
        return "support: tool '" + tool + "' needs transport 'batched' or 'single', got '" + str(record["transport"]) + "'"
    return ""

def support_records_error(records):
    """Validates support records and rejects duplicate tool cells."""
    seen = {}
    for record in records:
        err = support_record_error(record)
        if err != "":
            return err
        cell = record["tool"] + " " + record["capability"]
        if cell in seen:
            return "support: duplicate record for '" + cell + "'"
        seen[cell] = True
    return ""

def support_schema_error():
    """Validates every support record and rejects duplicate tool cells."""
    if SUPPORT_SCHEMA_VERSION != 1:
        return "support: unsupported schema v" + str(SUPPORT_SCHEMA_VERSION) + " (want v1)"
    return support_records_error(ALL_TOOL_SUPPORT)

def _qualified_records():
    """Returns the support records carrying a qualified state."""
    return [record for record in ALL_TOOL_SUPPORT if record["state"] == "qualified"]

def _wiring_gap(record, wired_tools):
    """Reports the actionable aspect gap for one qualified record."""
    if record["tool"] in wired_tools:
        return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no wired aspect (missing real_" + record["family"] + "_" + record["capability"] + "_aspect; not silently clean)"

def support_wiring_error(wired_tools):
    """Reports the actionable aspect gaps for the migrated families."""
    return "\n".join([gap for gap in [_wiring_gap(record, wired_tools) for record in _qualified_records()] if gap != ""])

def _cli_gap(record, cli_aspects):
    """Reports the actionable CLI gap for one qualified record."""
    for aspect in cli_aspects:
        if "real_" + record["family"] + "_" + record["capability"] in aspect:
            return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no CLI " + record["capability"] + " aspect (missing real_" + record["family"] + "_" + record["capability"] + "_aspect)"

def support_cli_error(cli_aspects):
    """Reports the actionable CLI gaps for the migrated families."""
    return "\n".join([gap for gap in [_cli_gap(record, cli_aspects) for record in _qualified_records()] if gap != ""])

def _artifact_gap(record):
    """Reports the actionable artifact gap for one qualified record."""
    if record["artifact"] == "pinned":
        return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no pinned artifact (quality/artifacts/" + record["tool"] + ".*.bzl is missing)"

def support_artifact_error():
    """Reports the actionable artifact gaps for the migrated families."""
    return "\n".join([gap for gap in [_artifact_gap(record) for record in _qualified_records()] if gap != ""])

def is_support_tool(tool_id):
    """Reports whether a tool carries an explicit support record."""
    for record in ALL_TOOL_SUPPORT:
        if tool_id == record["tool"]:
            return True
    return False

def support_state(tool_id):
    """Returns the recorded support state for one tool, or unknown."""
    for record in ALL_TOOL_SUPPORT:
        if tool_id == record["tool"]:
            return record["state"]
    return "unknown"
