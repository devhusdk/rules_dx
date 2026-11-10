"""Quality tool rows for the dockerfile family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "hadolint",
        adapters = {"lint": ["dockerfile"]},
        config = config(suffixes = [".yaml", ".yml"], tier_active = True),
        severity = severity(default = "warning", categories = {"DL1": "error"}),
        support = [
            row(
                capability = "lint",
                classes = ["dockerfile"],
                family = "dockerfile",
                fix = "check_only",
            ),
        ],
    ),
]
