"""Quality tool rows for the xml family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "xmllint",
        adapters = {"lint": ["xml"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "lint",
                classes = ["xml"],
                family = "xml",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
