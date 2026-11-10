"""Quality tool rows for the yaml family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "yamlfmt",
        adapters = {"format": ["ansible", "github_workflow", "kubernetes", "yaml"]},
        config = config(
            suffixes = [".yaml", ".yml", ".yamlfmt"],
            files = [".yamlfmt"],
            ext = "yaml",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["yaml"],
                family = "yaml",
                fix = "supported",
            ),
            row(
                capability = "format",
                classes = ["github_workflow"],
                family = "core",
                fix = "supported",
            ),
            row(
                capability = "format",
                classes = ["ansible"],
                family = "ansible",
                fix = "supported",
            ),
            row(
                capability = "format",
                classes = ["kubernetes"],
                family = "kubernetes",
                fix = "supported",
            ),
        ],
    ),
    tool(
        id = "yamllint",
        adapters = {"lint": ["yaml"]},
        config = config(
            suffixes = [".yaml", ".yml", ".yamllint"],
            files = [".yamllint"],
            ext = "yaml",
            tier_active = True,
            rule_keys = ["rules"],
        ),
        severity = severity(
            default = "warning",
            categories = {
                "anchors": "error",
                "key-duplicates": "error",
                "syntax": "error",
            },
        ),
        support = [
            row(
                capability = "lint",
                classes = ["yaml"],
                family = "yaml",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
