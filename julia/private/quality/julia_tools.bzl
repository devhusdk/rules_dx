"""Quality tool rows for the julia family."""

load("//core/quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "jet",
        adapters = {"typecheck": ["julia"]},
    ),
    tool(
        id = "julia_format",
        adapters = {"format": ["julia"]},
        support = [
            row(
                capability = "format",
                classes = ["julia"],
                context = "target_coupled",
                family = "julia",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                transport = "single",
                unsupported = {
                    "windows_arm64": "Julia ships no windows_arm64 runtime",
                },
            ),
        ],
    ),
    tool(
        id = "julialint",
        adapters = {"lint": ["julia"]},
        config = config(suffixes = [".toml"], tier_active = True, tier_base = False),
        support = [
            row(
                capability = "lint",
                classes = ["julia"],
                family = "julia",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                transport = "single",
                unsupported = {
                    "windows_arm64": "Julia ships no windows_arm64 runtime",
                },
            ),
        ],
    ),
]
