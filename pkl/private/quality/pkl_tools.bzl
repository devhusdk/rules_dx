"""Quality tool rows for the pkl family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "pkl",
        adapters = {
            "format": ["pkl"],
            "typecheck": ["pkl"],
        },
        support = [
            row(
                capability = "format",
                classes = ["pkl"],
                family = "pkl",
                fix = "supported",
                state = "qualified",
            ),
            row(
                capability = "typecheck",
                classes = ["pkl"],
                context = "target_coupled",
                family = "pkl",
                fix = "check_only",
                state = "qualified",
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "pkl_lsp",
        adapters = {"lint": ["pkl"]},
        config = config(kind = "none", tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["pkl"],
                family = "pkl",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64", "windows_arm64"],
            ),
        ],
    ),
]
