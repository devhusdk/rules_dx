"""Quality tool rows for the lua family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "luals",
        adapters = {"typecheck": ["lua"]},
        config = config(suffixes = [".json"], tier_active = True),
        severity = severity(default = "warning", categories = {
            "luals/assign-type-mismatch": "error",
            "luals/cast-type-mismatch": "error",
            "luals/error": "error",
            "luals/param-type-mismatch": "error",
            "luals/return-type-mismatch": "error",
        }),
    ),
    tool(
        id = "selene",
        adapters = {"lint": ["lua"]},
        config = config(suffixes = [".toml"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["lua"],
                family = "lua",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "stylua",
        adapters = {"format": ["lua"]},
        config = config(suffixes = [".toml"], tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["lua"],
                family = "lua",
                fix = "supported",
                state = "qualified",
            ),
        ],
    ),
]
