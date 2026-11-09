"""Quality tool rows for the zig family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "zig",
        adapters = {"typecheck": ["zig"]},
    ),
    tool(
        id = "zig_ast_check",
        adapters = {"lint": ["zig"]},
    ),
    tool(
        id = "zig_fmt",
        adapters = {"format": ["zig"]},
        config = config(kind = "none", tier_active = True),
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
