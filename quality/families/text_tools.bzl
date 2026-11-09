"""Quality tool rows for the text family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "docstrfmt",
        adapters = {"format": ["rst"]},
        config = config(
            suffixes = [".toml"],
            files = ["docstrfmt.toml"],
            ext = "toml",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["rst"],
                family = "rst",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "keep_sorted",
        adapters = {"lint": ["text"]},
        concerns = {"lint": "sorting"},
        support = [
            row(
                capability = "lint",
                classes = ["text"],
                family = "text",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "vale",
        adapters = {"lint": ["asciidoc", "markdown", "rst"]},
        concerns = {"lint": "prose"},
        config = config(
            suffixes = [".ini"],
            files = [".vale.ini", "_vale.ini"],
            references = ["ini:StylesPath"],
            tier_active = True,
        ),
        support = [
            row(
                capability = "lint",
                classes = ["asciidoc"],
                family = "asciidoc",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            row(
                capability = "lint",
                classes = ["markdown"],
                family = "markdown",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            row(
                capability = "lint",
                classes = ["rst"],
                family = "rst",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
