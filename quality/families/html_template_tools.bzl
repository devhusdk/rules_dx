"""Quality tool rows for the html_template family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "djlint",
        adapters = {
            "format": ["html_template"],
            "lint": ["html_template"],
        },
        config = config(suffixes = ["djlint.toml", ".djlintrc"], tier_active = True, tier_base = False),
        support = [
            row(
                capability = "format",
                classes = ["html_template"],
                family = "html_template",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
            row(
                capability = "lint",
                classes = ["html_template"],
                family = "html_template",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
]
