"""Quality tool rows for the sql family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "sqlfluff",
        adapters = {
            "format": ["sql"],
            "lint": ["sql"],
        },
        config = config(
            suffixes = [".sqlfluff", ".cfg", ".ini"],
            files = [".sqlfluff"],
            ext = "cfg",
            tier_active = True,
        ),
        severity = severity(
            default = "warning",
            categories = {
                "AM": "error",
                "LXR": "error",
                "PRS": "error",
                "RF": "error",
                "ST": "error",
                "TMP": "error",
            },
        ),
        support = [
            row(
                capability = "format",
                classes = ["sql"],
                family = "sql",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
            ),
            row(
                capability = "lint",
                classes = ["sql"],
                family = "sql",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
