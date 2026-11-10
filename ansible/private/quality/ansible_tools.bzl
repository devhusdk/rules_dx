"""Quality tool rows for the ansible family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "ansible_lint",
        adapters = {"lint": ["ansible"]},
        config = config(suffixes = [".ansible-lint", ".yml", ".yaml"], tier_active = True, tier_source = "ansible-lint moderate and production profiles"),
        severity = severity(default = "warning", categories = {"internal-error": "error", "load-failure": "error", "syntax-check": "error"}),
        support = [
            row(
                capability = "lint",
                classes = ["ansible"],
                family = "ansible",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                unsupported = {
                    "windows_arm64": "ansible-core does not run on Windows",
                    "windows_x86_64": "ansible-core does not run on Windows",
                },
            ),
        ],
    ),
]
