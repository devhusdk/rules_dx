"""Quality tool rows for the dockerfile family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "hadolint",
        adapters = {"lint": ["dockerfile"]},
        config = config(suffixes = [".yaml", ".yml"], tier_active = True),
        severity = severity(default = "warning", categories = {
            "DL1": "error",
            "DL3002": "error",
            "DL3006": "error",
            "DL3007": "error",
            "DL3008": "error",
            "DL3018": "error",
            "DL4006": "error",
        }),
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
