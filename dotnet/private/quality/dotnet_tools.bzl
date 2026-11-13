"""Quality tool rows for the dotnet family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "csc",
        adapters = {"typecheck": ["csharp"]},
        severity = severity(default = "error"),
        support = [
            row(
                capability = "typecheck",
                classes = ["csharp"],
                context = "target_coupled",
                family = "csharp",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "csharpier",
        adapters = {"format": ["csharp"]},
        config = config(suffixes = [".yaml"], tier_active = True, tier_base = False),
        support = [
            row(
                capability = "format",
                classes = ["csharp"],
                family = "csharp",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "fantomas",
        adapters = {"format": ["fsharp"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["fsharp"],
                family = "fsharp",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "fsc",
        adapters = {"typecheck": ["fsharp"]},
        severity = severity(default = "error"),
        support = [
            row(
                capability = "typecheck",
                classes = ["fsharp"],
                context = "target_coupled",
                family = "fsharp",
                fix = "check_only",
                platforms = ["linux_x86_64"],
            ),
        ],
    ),
    tool(
        id = "fsharplint",
        adapters = {"lint": ["fsharp"]},
        config = config(suffixes = [".json"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["fsharp"],
                context = "target_coupled",
                family = "fsharp",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "roslyn",
        adapters = {"lint": ["csharp"]},
        config = config(suffixes = [".globalconfig"], tier_active = True),
        severity = severity(default = "warning", categories = {
            "CA2100": "error",
            "CA2109": "error",
            "CA2119": "error",
            "CA2153": "error",
            "CA23": "error",
            "CA3": "error",
            "CA5": "error",
        }),
        support = [
            row(
                capability = "lint",
                classes = ["csharp"],
                context = "target_coupled",
                family = "csharp",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
