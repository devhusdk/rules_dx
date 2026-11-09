"""Quality tool rows for the jsonnet family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "jsonnet_lint",
        adapters = {"lint": ["jsonnet"]},
        config = config(kind = "none", tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["jsonnet"],
                family = "jsonnet",
                fix = "check_only",
            ),
        ],
    ),
    tool(
        id = "jsonnetfmt",
        adapters = {"format": ["jsonnet"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["jsonnet"],
                family = "jsonnet",
                fix = "supported",
                state = "qualified",
            ),
        ],
    ),
]
