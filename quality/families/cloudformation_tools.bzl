"""Quality tool rows for the cloudformation family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "cfn_lint",
        adapters = {"lint": ["cloudformation"]},
        config = config(suffixes = [".cfnlintrc", ".yaml", ".yml"], tier_active = True),
        support = [
            row(
                capability = "lint",
                classes = ["cloudformation"],
                family = "cloudformation",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
