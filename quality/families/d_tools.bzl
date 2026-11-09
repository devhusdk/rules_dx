"""Quality tool rows for the d family."""

load("//quality:tool_defs.bzl", "tool")

TOOLS = [
    tool(
        id = "ldc",
        adapters = {"typecheck": ["d"]},
    ),
]
