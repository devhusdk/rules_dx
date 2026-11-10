"""Quality tool rows for the shell family."""

load("//core/quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "shellcheck",
        adapters = {"lint": ["shell"]},
        config = config(suffixes = [".shellcheckrc", "shellcheckrc"], files = [".shellcheckrc"], tier_active = True),
        support = [
            row(
                capability = "lint",
                classes = ["shell"],
                family = "shell",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "shfmt",
        adapters = {"format": ["shell"]},
        config = config(
            suffixes = [".editorconfig"],
            files = [".editorconfig"],
            ext = "editorconfig",
            comment = "#",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["shell"],
                family = "shell",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                state = "qualified",
                transport = "single",
            ),
        ],
    ),
]
