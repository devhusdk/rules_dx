"""Quality tool rows for the bicep family."""

load("//core/quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "bicep",
        adapters = {
            "format": ["bicep"],
            "lint": ["bicep"],
        },
        config = config(suffixes = ["bicepconfig.json"], files = ["bicepconfig.json"], tier_active = True, stage_native = True),
        support = [
            row(
                capability = "format",
                classes = ["bicep"],
                family = "bicep",
                fix = "supported",
                transport = "single",
            ),
            row(
                capability = "lint",
                classes = ["bicep"],
                context = "target_coupled",
                family = "bicep",
                fix = "check_only",
                transport = "single",
            ),
        ],
    ),
]
