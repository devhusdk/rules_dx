"""Quality tool rows for the swift family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "swift_format",
        adapters = {"format": ["swift"]},
        config = config(suffixes = [".swift-format", ".json"], tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["swift"],
                context = "target_coupled",
                family = "swift",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                transport = "single",
                unsupported = {
                    "windows_arm64": "swift-format is pinned only for Linux and macOS arm64",
                    "windows_x86_64": "swift-format is pinned only for Linux and macOS arm64",
                },
            ),
        ],
    ),
    tool(
        id = "swift_lint",
        adapters = {"lint": ["swift"]},
        config = config(suffixes = [".swift-format", ".json"], tier_active = True),
        support = [
            row(
                capability = "lint",
                classes = ["swift"],
                context = "target_coupled",
                family = "swift",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                unsupported = {
                    "windows_arm64": "swift_lint runs the swift-format pinned only for Linux and macOS arm64",
                    "windows_x86_64": "swift_lint runs the swift-format pinned only for Linux and macOS arm64",
                },
            ),
        ],
    ),
]
