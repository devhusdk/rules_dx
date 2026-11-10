"""Quality tool rows for the perl family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "perlcritic",
        adapters = {"lint": ["perl"]},
        config = config(suffixes = [".perlcriticrc", "perlcriticrc"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["perl"],
                family = "perl",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "Windows ARM64 perl is not qualified",
                },
            ),
        ],
    ),
    tool(
        id = "perltidy",
        adapters = {"format": ["perl"]},
        config = config(
            suffixes = [".perltidyrc", "perltidyrc"],
            files = [".perltidyrc"],
            ext = "perltidyrc",
            comment = "#",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["perl"],
                family = "perl",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "Windows ARM64 perl is not qualified",
                },
            ),
        ],
    ),
]
