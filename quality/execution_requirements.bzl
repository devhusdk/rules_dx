"""Per-tool remote-execution qualification evidence (default local-only)."""

DX_NO_REMOTE_EXEC = "no-remote-exec"

DX_FORBIDDEN_NO_REMOTE = "no-remote"

DX_HOST_EXECUTION_PLATFORM = "host"

DX_REMOTE_CAPABILITIES = ["audit", "format", "lint", "typecheck"]

REMOTE_QUALIFIED_STAGES = {}

RUNNER_REMOTE_QUALIFIED = False

def _is_canonical_token(text):
    """Reports whether a token uses the canonical alphabet."""
    if text == "":
        return False
    for c in text.elems():
        if c not in "abcdefghijklmnopqrstuvwxyz0123456789_":
            return False
    return True

def dx_remote_stage_key(tool, capability, exec_platform):
    """Builds the qualification key for one tool, capability and platform."""
    return tool + ":" + capability + ":" + exec_platform

def dx_remote_stage_error(tool, capability, exec_platform, qualified_stages):
    """Reports why one stage stays local-only, or empty when qualified."""
    if not _is_canonical_token(tool):
        return "remote: stage names non-canonical tool '" + str(tool) + "'"
    if capability not in DX_REMOTE_CAPABILITIES:
        return "remote: stage names unknown capability '" + str(capability) + "'"
    if not _is_canonical_token(exec_platform):
        return "remote: stage names non-canonical platform '" + str(exec_platform) + "'"
    if dx_remote_stage_key(tool, capability, exec_platform) not in qualified_stages:
        return "remote: stage '" + tool + " " + capability + " " + exec_platform + "' is not qualified"
    return ""

def dx_remote_table_error(qualified_stages):
    """Reports the first malformed qualification record, or empty."""
    for key in sorted(qualified_stages.keys()):
        parts = key.split(":")
        if len(parts) != 3:
            return "remote: malformed qualification record '" + key + "'"
        if not _is_canonical_token(parts[0]):
            return "remote: record names non-canonical tool '" + key + "'"
        if parts[1] not in DX_REMOTE_CAPABILITIES:
            return "remote: record names unknown capability '" + key + "'"
        if not _is_canonical_token(parts[2]):
            return "remote: record names non-canonical platform '" + key + "'"
    return ""

def dx_action_remote_error(stage_tools, capability, exec_platform, qualified_stages, runner_qualified):
    """Reports why one action stays local-only, or empty when remote-capable."""
    if len(stage_tools) == 0:
        return "remote: action selects no stages"
    if not runner_qualified:
        return "remote: shared runner is not qualified"
    for tool in sorted(stage_tools):
        error = dx_remote_stage_error(tool, capability, exec_platform, qualified_stages)
        if error != "":
            return error
    return ""

def dx_action_requirements(stage_tools, capability, exec_platform = DX_HOST_EXECUTION_PLATFORM, qualified_stages = REMOTE_QUALIFIED_STAGES, runner_qualified = RUNNER_REMOTE_QUALIFIED):
    """Returns remote-capable or local-only requirements for one action."""
    if dx_action_remote_error(stage_tools, capability, exec_platform, qualified_stages, runner_qualified) == "":
        return {}
    return {DX_NO_REMOTE_EXEC: "1"}

def dx_execution_requirements():
    """Returns the default local-only requirements for one Dx action."""
    return dx_action_requirements([], "")

def dx_is_local_only(requirements):
    """Reports whether requirements carry the local-only marker."""
    return requirements.get(DX_NO_REMOTE_EXEC, "") == "1"

def dx_has_forbidden_marker(requirements):
    """Reports whether requirements carry the cache-disabling marker."""
    return DX_FORBIDDEN_NO_REMOTE in requirements
