"""Quality tool rows for the typst family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "tinymist",
        adapters = {"lint": ["typst"]},
        config = config(kind = "none", tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["typst"],
                family = "typst",
                fix = "check_only",
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "typstyle",
        adapters = {"format": ["typst"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["typst"],
                family = "typst",
                fix = "supported",
            ),
        ],
    ),
]
