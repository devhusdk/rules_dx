"""Local-only execution requirements helper with per-tool remote qualification."""

load("//core/quality:adapters.bzl", "REAL_ADAPTERS")
load("//core/quality/artifacts:hub.bzl", "TOOL_PLATFORMS")

visibility("private")

DX_NO_REMOTE_EXEC = "no-remote-exec"

DX_FORBIDDEN_NO_REMOTE = "no-remote"

def _qualified(tool, capability):
    """One remote record for a pinned single-binary tool on every qualified platform."""
    return {
        "capability": capability,
        "closure": "declared",
        "platforms": list(TOOL_PLATFORMS),
        "runner": "hermetic",
        "state": "qualified",
        "tool": tool,
    }

SHFMT_FORMAT_REMOTE = _qualified("shfmt", "format")

# A tool qualifies when its action reads only declared inputs: a pinned @dx_tools
# binary built for the exec platform (cfg = "exec"), the staged sources and configs,
# and the runner's hermetic env (no HOME, no host PATH, no network). A stage runs
# remotely only when every tool in it is listed here, see dx_stage_execution_requirements.
REMOTE_QUALIFIED = [
    SHFMT_FORMAT_REMOTE,
    _qualified("biome", "format"),
    _qualified("biome", "lint"),
    _qualified("buildifier", "format"),
    _qualified("buildifier", "lint"),
    _qualified("gersemi", "format"),
    _qualified("jsonnetfmt", "format"),
    _qualified("keep_sorted", "lint"),
    _qualified("shellcheck", "lint"),
    _qualified("stylua", "format"),
    _qualified("taplo", "format"),
    _qualified("typos", "lint"),
    _qualified("typstyle", "format"),
    _qualified("yamlfmt", "format"),
]

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
