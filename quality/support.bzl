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

SCALA_SCALAFMT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["scala"],
    "context": "owned",
    "execution": "direct",
    "family": "scala",
    "fix": "supported",
    "platforms": ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
    "state": "supported",
    "tool": "scalafmt",
    "transport": "batched",
}

SCALA_SCALAFIX_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["scala"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "scala",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "scalafix",
    "transport": "batched",
}

CSHARP_CSHARPIER_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["csharp"],
    "context": "owned",
    "execution": "direct",
    "family": "csharp",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "csharpier",
    "transport": "batched",
}

FSHARP_FANTOMAS_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["fsharp"],
    "context": "owned",
    "execution": "direct",
    "family": "fsharp",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "fantomas",
    "transport": "batched",
}

FSHARP_FSHARPLINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["fsharp"],
    "context": "target_coupled",
    "execution": "direct",
    "family": "fsharp",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "fsharplint",
    "transport": "batched",
}

TERRAFORM_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["terraform"],
    "context": "owned",
    "execution": "direct",
    "family": "terraform",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "qualified",
    "tool": "terraform",
    "transport": "single",
}

PROTOBUF_BUF_FORMAT_SUPPORT = {
    "artifact": "pinned",
    "capability": "format",
    "classes": ["protobuf"],
    "context": "owned",
    "execution": "direct",
    "family": "protobuf",
    "fix": "supported",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "buf",
    "transport": "batched",
}

PROTOBUF_BUF_LINT_SUPPORT = {
    "artifact": "pinned",
    "capability": "lint",
    "classes": ["protobuf"],
    "context": "owned",
    "execution": "direct",
    "family": "protobuf",
    "fix": "check_only",
    "platforms": ["linux_x86_64"],
    "state": "supported",
    "tool": "buf",
    "transport": "batched",
}

SUPPORT_RECORDS = [
    SCALA_SCALAFMT_SUPPORT,
    SCALA_SCALAFIX_SUPPORT,
    CSHARP_CSHARPIER_SUPPORT,
    FSHARP_FANTOMAS_SUPPORT,
    FSHARP_FSHARPLINT_SUPPORT,
    TEXT_KEEP_SORTED_SUPPORT,
    SHELL_SHELLCHECK_SUPPORT,
    SHELL_SHFMT_SUPPORT,
    TERRAFORM_FORMAT_SUPPORT,
    PROTOBUF_BUF_FORMAT_SUPPORT,
    PROTOBUF_BUF_LINT_SUPPORT,
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
    if record["context"] not in ["owned", "target_coupled"]:
        return "support: tool '" + tool + "' needs context 'owned' or 'target_coupled', got '" + str(record["context"]) + "'"
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

def support_schema_error():
    """Validates every recorded support entry."""
    for record in SUPPORT_RECORDS:
        err = support_record_error(record)
        if err != "":
            return err
    return ""

def _wiring_error(record, wired_tools, aspect):
    """Reports the missing wired aspect for one qualified record."""
    if record["tool"] in wired_tools:
        return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no wired aspect (missing " + aspect + "; not silently clean)"

def support_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the migrated family."""
    return _wiring_error(TEXT_KEEP_SORTED_SUPPORT, wired_tools, "real_text_lint_aspect")

def support_shell_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the shell family."""
    for record in [SHELL_SHELLCHECK_SUPPORT, SHELL_SHFMT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_shell_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_scala_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Scala family."""
    for record in [SCALA_SCALAFMT_SUPPORT, SCALA_SCALAFIX_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_scala_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_terraform_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Terraform family."""
    return _wiring_error(TERRAFORM_FORMAT_SUPPORT, wired_tools, "real_terraform_format_aspect")

def support_protobuf_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the Protocol Buffers family."""
    for record in [PROTOBUF_BUF_FORMAT_SUPPORT, PROTOBUF_BUF_LINT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_protobuf_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_csharp_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the C# family."""
    return _wiring_error(CSHARP_CSHARPIER_SUPPORT, wired_tools, "real_dotnet_format_aspect")

def support_fsharp_wiring_error(wired_tools):
    """Reports the actionable aspect gap for the F# family."""
    for record in [FSHARP_FANTOMAS_SUPPORT, FSHARP_FSHARPLINT_SUPPORT]:
        err = _wiring_error(record, wired_tools, "real_dotnet_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def _cli_error(record, cli_aspects, aspect):
    """Reports the missing CLI aspect for one qualified record."""
    for seen in cli_aspects:
        if aspect in seen:
            return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no CLI " + record["capability"] + " aspect (missing " + aspect + ")"

def support_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the migrated family."""
    return _cli_error(TEXT_KEEP_SORTED_SUPPORT, cli_aspects, "real_text_lint_aspect")

def support_shell_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the shell family."""
    for record in [SHELL_SHELLCHECK_SUPPORT, SHELL_SHFMT_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_shell_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_scala_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Scala family."""
    for record in [SCALA_SCALAFMT_SUPPORT, SCALA_SCALAFIX_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_scala_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_terraform_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Terraform family."""
    return _cli_error(TERRAFORM_FORMAT_SUPPORT, cli_aspects, "real_terraform_format_aspect")

def support_protobuf_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the Protocol Buffers family."""
    for record in [PROTOBUF_BUF_FORMAT_SUPPORT, PROTOBUF_BUF_LINT_SUPPORT]:
        err = _cli_error(record, cli_aspects, "real_protobuf_" + record["capability"] + "_aspect")
        if err != "":
            return err
    return ""

def support_csharp_cli_error(cli_aspects):
    """Reports the actionable CLI gap for the C# family."""
    return _cli_error(CSHARP_CSHARPIER_SUPPORT, cli_aspects, "real_dotnet_format_aspect")

def _artifact_error(record):
    """Reports the missing pinned artifact for one qualified record."""
    if record["artifact"] == "pinned":
        return ""
    return "support: " + record["tool"] + " " + record["capability"] + " is qualified but has no pinned artifact (quality/artifacts/" + record["tool"] + ".*.bzl is missing)"

def support_artifact_error():
    """Reports the actionable artifact gap for the migrated family."""
    return _artifact_error(TEXT_KEEP_SORTED_SUPPORT)

def support_shell_artifact_error():
    """Reports the actionable artifact gap for the shell family."""
    for record in [SHELL_SHELLCHECK_SUPPORT, SHELL_SHFMT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_scala_artifact_error():
    """Reports the actionable artifact gap for the Scala family."""
    for record in [SCALA_SCALAFMT_SUPPORT, SCALA_SCALAFIX_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_terraform_artifact_error():
    """Reports the actionable artifact gap for the Terraform family."""
    return _artifact_error(TERRAFORM_FORMAT_SUPPORT)

def support_protobuf_artifact_error():
    """Reports the actionable artifact gap for the Protocol Buffers family."""
    for record in [PROTOBUF_BUF_FORMAT_SUPPORT, PROTOBUF_BUF_LINT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def support_csharp_artifact_error():
    """Reports the actionable artifact gap for the C# family."""
    return _artifact_error(CSHARP_CSHARPIER_SUPPORT)

def support_fsharp_artifact_error():
    """Reports the actionable artifact gap for the F# family."""
    for record in [FSHARP_FANTOMAS_SUPPORT, FSHARP_FSHARPLINT_SUPPORT]:
        err = _artifact_error(record)
        if err != "":
            return err
    return ""

def is_support_tool(tool_id):
    """Reports whether a tool carries an explicit support record."""
    for record in SUPPORT_RECORDS:
        if tool_id == record["tool"]:
            return True
    return False

def support_state(tool_id):
    """Returns the recorded support state for one tool, or unknown."""
    for record in SUPPORT_RECORDS:
        if tool_id == record["tool"]:
            return record["state"]
    return "unknown"
