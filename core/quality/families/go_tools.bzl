"""Quality tool rows for the go family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "errcheck",
        adapters = {"lint": ["go"]},
        concerns = {"lint": "unchecked_errors"},
        config = config(suffixes = [".txt"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["go"],
                context = "target_coupled",
                family = "go",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "gofumpt",
        adapters = {"format": ["go"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["go"],
                family = "go",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "gotype",
        adapters = {"typecheck": ["go"]},
        severity = severity(default = "error"),
        support = [
            row(
                capability = "typecheck",
                classes = ["go"],
                context = "target_coupled",
                family = "go",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "govet",
        adapters = {"lint": ["go"]},
        concerns = {"lint": "vet"},
        config = config(suffixes = [".json"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["go"],
                context = "target_coupled",
                family = "go",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "modfmt",
        adapters = {"format": ["go_module"]},
        support = [
            row(
                capability = "format",
                classes = ["go_module"],
                family = "go_module",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "staticcheck",
        adapters = {"lint": ["go"]},
        config = config(suffixes = [".conf"], files = ["staticcheck.conf"], tier_active = True, stage_native = True),
        severity = severity(categories = {
            "QF": "info",
            "S": "warning",
            "U1000": "warning",
        }),
        support = [
            row(
                capability = "lint",
                classes = ["go"],
                context = "target_coupled",
                family = "go",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "known",
            ),
        ],
    ),
]
