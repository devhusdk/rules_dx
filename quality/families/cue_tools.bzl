"""Quality tool rows for the cue family."""

load("//quality:tool_defs.bzl", "row", "tool")

TOOLS = [
    tool(
        id = "cue",
        adapters = {
            "format": ["cue"],
            "typecheck": ["cue"],
        },
        support = [
            row(
                capability = "format",
                classes = ["cue"],
                family = "cue",
                fix = "supported",
                state = "qualified",
            ),
            row(
                capability = "typecheck",
                classes = ["cue"],
                context = "target_coupled",
                family = "cue",
                fix = "check_only",
                state = "qualified",
            ),
        ],
    ),
]
