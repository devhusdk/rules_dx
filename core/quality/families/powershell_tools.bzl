"""Quality tool rows for the powershell family."""

load("//core/quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "psscriptanalyzer",
        adapters = {
            "format": ["powershell"],
            "lint": ["powershell"],
        },
        config = config(
            suffixes = [".psd1"],
            files = ["PSScriptAnalyzerSettings.psd1"],
            ext = "psd1",
            comment = "#",
            init_order = 6,
            family = "powershell",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["powershell"],
                family = "powershell",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            row(
                capability = "lint",
                classes = ["powershell"],
                family = "powershell",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
