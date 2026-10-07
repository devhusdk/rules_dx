"""Deterministic versioned quality action request serialization."""

_REQUEST_VERSION = 1

def _mapping(workspace, exec_path):
    """Renders one workspace-to-exec mapping entry."""
    return {"exec": exec_path, "workspace": workspace}

def quality_request_json(
        producer,
        capability,
        output,
        stages,
        sources,
        siblings,
        resolves,
        tool_binaries,
        tool_configs,
        tool_editions,
        tool_files,
        tool_env,
        upstream_diagnostics,
        real = False,
        scratch_parent = None):
    """Serializes one quality action request to deterministic JSON."""
    request = {
        "capability": capability,
        "output": output,
        "producer": producer,
        "real": real,
        "resolves": [_mapping(workspace, exec_path) for (workspace, exec_path) in resolves],
        "scratch_parent": scratch_parent,
        "siblings": [_mapping(workspace, exec_path) for (workspace, exec_path) in siblings],
        "sources": [_mapping(workspace, exec_path) for (workspace, exec_path) in sources],
        "stages": [
            {
                "classes": stage["classes"],
                "sources": stage["sources"],
                "tool": stage["tool"],
            }
            for stage in stages
        ],
        "tool_binaries": [{"path": path, "tool": tool} for (tool, path) in tool_binaries],
        "tool_configs": [{"rel": rel, "tool": tool} for (tool, rel) in tool_configs],
        "tool_editions": [{"edition": edition, "tool": tool} for (tool, edition) in tool_editions],
        "tool_env": [
            {"key": key, "tool": tool, "value": value}
            for (tool, key, value) in tool_env
        ],
        "tool_files": [
            {"exec": exec_path, "rel": rel, "tool": tool}
            for (tool, rel, exec_path) in tool_files
        ],
        "upstream_diagnostics": [
            {"exec": exec_path, "tool": tool}
            for (tool, exec_path) in upstream_diagnostics
        ],
        "version": _REQUEST_VERSION,
    }
    return json.encode(request)
