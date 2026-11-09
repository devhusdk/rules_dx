"""Quality tool rows for the dart family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "dart",
        adapters = {
            "format": ["dart"],
            "lint": ["dart"],
            "typecheck": ["dart"],
        },
        config = config(
            suffixes = [".yaml"],
            files = ["analysis_options.yaml"],
            ext = "yaml",
            extends_syntax = "include: {base}\n",
            comment = "#",
            init_order = 5,
            family = "dart",
            tier_active = True,
        ),
        support = [
            row(
                capability = "lint",
                classes = ["dart"],
                context = "target_coupled",
                family = "dart",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "rules_dart ships no windows_arm64 SDK",
                },
            ),
            row(
                capability = "format",
                classes = ["dart"],
                context = "target_coupled",
                family = "dart",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "rules_dart ships no windows_arm64 SDK",
                },
            ),
            row(
                capability = "typecheck",
                classes = ["dart"],
                context = "target_coupled",
                family = "dart",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "rules_dart ships no windows_arm64 SDK",
                },
            ),
        ],
    ),
]
