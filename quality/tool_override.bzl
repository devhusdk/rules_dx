"""Consumer-owned quality-tool executable overrides."""

load("//quality:adapters.bzl", "REAL_ADAPTERS")

DxQualityToolInfo = provider(
    doc = "Consumer-owned executable override for one real tool.",
    fields = {
        "tool": "File: the replacement executable run in place of the managed default.",
        "tool_id": "str: stable built-in tool identifier from REAL_ADAPTERS.",
        "version": "str: consumer-declared identity of the replacement build.",
    },
)

def tool_override_error(tool_id, version, has_tool):
    """Returns the validation error for a tool override, or "" when valid."""
    if tool_id not in REAL_ADAPTERS:
        return "tool_override: unknown tool '" + tool_id + "': want one of " + ", ".join(sorted(REAL_ADAPTERS.keys()))
    if version == "":
        return "tool_override (" + tool_id + "): version is required; declare the replacement build identity"
    if not has_tool:
        return "tool_override (" + tool_id + "): tool is required; declare the replacement executable"
    return ""

def collect_tool_overrides(hints, stage_tools, what):
    """Resolves tool override hints to the overrides for a pipeline's stage tools."""
    by_tool = {}
    for hint in hints:
        if hint.tool_id not in REAL_ADAPTERS:
            fail("tool_override (" + what + "): unknown tool '" + hint.tool_id + "'")
        if hint.tool_id in by_tool:
            fail("tool_override (" + what + "): duplicate aspect_hints for tool '" + hint.tool_id + "'")
        by_tool[hint.tool_id] = hint
    return {tool: by_tool[tool] for tool in stage_tools if tool in by_tool}

def _tool_override_impl(ctx):
    err = tool_override_error(ctx.attr.tool_id, ctx.attr.version, ctx.file.tool != None)
    if err != "":
        fail(err + " (in " + str(ctx.label) + ")")
    tool = ctx.file.tool
    runfiles = ctx.runfiles(files = [tool] + ctx.files.data)
    return [
        DefaultInfo(
            files = depset([tool] + ctx.files.data),
            runfiles = runfiles,
        ),
        DxQualityToolInfo(
            tool = tool,
            tool_id = ctx.attr.tool_id,
            version = ctx.attr.version,
        ),
    ]

quality_tool_override = rule(
    implementation = _tool_override_impl,
    attrs = {
        "data": attr.label_list(
            allow_files = True,
            default = [],
        ),
        "tool": attr.label(
            allow_single_file = True,
            cfg = "exec",
            mandatory = True,
        ),
        "tool_id": attr.string(
            mandatory = True,
        ),
        "version": attr.string(
            mandatory = True,
        ),
    },
)
