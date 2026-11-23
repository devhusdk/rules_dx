"""Quality tool rows for the zig family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "zig",
        adapters = {"typecheck": ["zig"]},
        severity = severity(default = "error"),
    ),
    tool(
        id = "zig_ast_check",
        adapters = {"lint": ["zig"]},
        severity = severity(default = "error"),
    ),
    tool(
        id = "zig_fmt",
        adapters = {"format": ["zig"]},
        config = config(kind = "none", tier_active = True),
        severity = severity(default = "error"),
        support = [
            row(
                capability = "format",
                classes = ["zig"],
                context = "target_coupled",
                family = "zig",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
                unsupported = {
                },
            ),
        ],
    ),
]
