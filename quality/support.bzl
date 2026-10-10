"""Explicit tool support records."""

load(":adapters.bzl", "REAL_ADAPTERS", "REAL_CLASS_TO_FAMILY")

SUPPORT_SCHEMA_VERSION = 1

TEXT_KEEP_SORTED_SUPPORT = {
    "artifact": "missing",
    "capability": "lint",
    "classes": ["text"],
    "context": "owned",
    "execution": "direct",
    "family": "text",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "known",
    "tool": "keep_sorted",
}

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
    for key in ["artifact", "capability", "classes", "context", "execution", "family", "fix", "platforms", "state", "tool"]:
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
    return ""

def support_schema_error():
    """Validates the versioned support-record schema."""
    return support_record_error(TEXT_KEEP_SORTED_SUPPORT)

def support_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the migrated family."""
    if TEXT_KEEP_SORTED_SUPPORT["tool"] in wired_tools:
        return ""
    return "support: keep_sorted lint is known but has no wired aspect (missing real_text_lint_aspect; not silently clean)"

def support_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the migrated family."""
    for aspect in cli_aspects:
        if "real_text_lint" in aspect:
            return ""
    return "support: keep_sorted lint is known but has no CLI lint aspect (missing real_text_lint_aspect)"

def support_artifact_error():
    """Reports the actionable artifact gap for the migrated family."""
    if TEXT_KEEP_SORTED_SUPPORT["artifact"] == "pinned":
        return ""
    return "support: keep_sorted lint is known but has no pinned artifact (quality/artifacts/keep_sorted.*.bzl is missing)"

def is_support_tool(tool_id):
    """Reports whether a tool carries an explicit support record."""
    return tool_id == TEXT_KEEP_SORTED_SUPPORT["tool"]

def support_state(tool_id):
    """Returns the recorded support state for one tool, or unknown."""
    if tool_id == TEXT_KEEP_SORTED_SUPPORT["tool"]:
        return TEXT_KEEP_SORTED_SUPPORT["state"]
    return "unknown"
