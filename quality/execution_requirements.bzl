"""Local-only execution requirements helper with per-tool remote qualification."""

load("//quality:adapters.bzl", "REAL_ADAPTERS")
load("//quality/artifacts:hub.bzl", "TOOL_PLATFORMS")

DX_NO_REMOTE_EXEC = "no-remote-exec"

DX_FORBIDDEN_NO_REMOTE = "no-remote"

SHFMT_FORMAT_REMOTE = {
    "capability": "format",
    "closure": "declared",
    "platforms": ["linux_x86_64", "linux_arm64", "macos_arm64", "windows_x86_64"],
    "runner": "hermetic",
    "state": "qualified",
    "tool": "shfmt",
}

REMOTE_QUALIFIED = [SHFMT_FORMAT_REMOTE]

def dx_is_local_only(requirements):
    """Reports whether requirements carry the local-only marker."""
    return requirements.get(DX_NO_REMOTE_EXEC, "") == "1"

def dx_has_forbidden_marker(requirements):
    """Reports whether requirements carry the cache-disabling marker."""
    return DX_FORBIDDEN_NO_REMOTE in requirements

def remote_record_error(record):
    """Validates one remote qualification record against the adapter taxonomy."""
    for key in ["capability", "closure", "platforms", "runner", "state", "tool"]:
        if key not in record:
            return "remote: record is missing field '" + key + "'"
    tool = record["tool"]
    capability = record["capability"]
    if tool not in REAL_ADAPTERS:
        return "remote: unknown tool '" + tool + "': not in the real adapter registry"
    if capability not in REAL_ADAPTERS[tool]:
        return "remote: tool '" + tool + "' has no '" + capability + "' capability"
    if record["closure"] != "declared":
        return "remote: tool '" + tool + "' needs closure 'declared', got '" + str(record["closure"]) + "'"
    if record["runner"] != "hermetic":
        return "remote: tool '" + tool + "' needs runner 'hermetic', got '" + str(record["runner"]) + "'"
    if record["state"] != "qualified":
        return "remote: tool '" + tool + "' needs state 'qualified', got '" + str(record["state"]) + "'"
    platforms = record["platforms"]
    if type(platforms) != "list" or len(platforms) == 0:
        return "remote: tool '" + tool + "' needs a non-empty execution platform list"
    for platform in platforms:
        if platform not in TOOL_PLATFORMS:
            return "remote: tool '" + tool + "' names unknown execution platform '" + str(platform) + "'"
    return ""

def remote_platform_error(record, artifact_platforms):
    """Reports the platforms a remote record claims without a pinned artifact."""
    covered = artifact_platforms.get(record["tool"], [])
    missing = [platform for platform in record["platforms"] if platform not in covered]
    if len(missing) > 0:
        return "remote: tool '" + record["tool"] + "' is remote-qualified for " + ", ".join(missing) + " without a pinned artifact"
    return ""

def dx_remote_platforms(tool, capability):
    """Returns the qualified execution platforms for one tool and capability."""
    for record in REMOTE_QUALIFIED:
        if record["tool"] == tool and record["capability"] == capability:
            return list(record["platforms"])
    return []

def dx_is_remote_qualified(tool, capability):
    """Reports whether one tool and capability carry a valid remote record."""
    for record in REMOTE_QUALIFIED:
        if record["tool"] == tool and record["capability"] == capability:
            return remote_record_error(record) == ""
    return False

def dx_stage_execution_requirements(stage_tools, capability):
    """Returns remote-capable requirements only when every stage is qualified."""
    if len(stage_tools) == 0:
        return {DX_NO_REMOTE_EXEC: "1"}
    for tool in stage_tools:
        if not dx_is_remote_qualified(tool, capability):
            return {DX_NO_REMOTE_EXEC: "1"}
    return {}
