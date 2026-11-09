"""Quality tool rows for the cmake family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "cmakelint",
        adapters = {"lint": ["cmake"]},
        config = config(
            suffixes = [".cmakelintrc"],
            files = [".cmakelintrc"],
            ext = "cmakelintrc",
            tier_active = True,
        ),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["cmake"],
                family = "cmake",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "gersemi",
        adapters = {"format": ["cmake"]},
        config = config(
            suffixes = [".gersemirc", ".yaml", ".yml"],
            files = [".gersemirc"],
            ext = "yaml",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["cmake"],
                family = "cmake",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
