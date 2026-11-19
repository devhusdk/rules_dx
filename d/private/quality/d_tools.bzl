"""Quality tool rows for the d family."""

load("//core/quality:tool_defs.bzl", "row", "tool")

TOOLS = [
    tool(
        id = "dscanner",
        adapters = {"lint": ["d"]},
        support = [
            row(
                capability = "lint",
                classes = ["d"],
                family = "d",
                fix = "check_only",
                platforms = ["linux_x86_64"],
            ),
        ],
    ),
    tool(
        id = "ldc",
        adapters = {"typecheck": ["d"]},
    ),
]
