"""Quality tool rows for the make family."""

load("//core/quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "checkmake",
        adapters = {"lint": ["make"]},
        config = config(
            suffixes = [".ini"],
            files = ["checkmake.ini"],
            ext = "ini",
            tier_active = True,
        ),
        support = [
            row(
                capability = "lint",
                classes = ["make"],
                family = "make",
                fix = "check_only",
            ),
        ],
    ),
    tool(
        id = "mbake",
        adapters = {"format": ["make"]},
        config = config(
            suffixes = [".bake.toml", ".toml"],
            files = [".bake.toml"],
            ext = "toml",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["make"],
                family = "make",
                fix = "supported",
                platforms = ["linux_x86_64"],
            ),
        ],
    ),
]
