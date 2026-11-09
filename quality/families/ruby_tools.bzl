"""Quality tool rows for the ruby family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "rubocop",
        adapters = {"format": ["ruby"], "lint": ["ruby"]},
        config = config(
            suffixes = [".yml", ".yaml"],
            tier_active = True,
            suppressions = ["rubocop:disable"],
            suppression_extensions = [".rake", ".rb"],
        ),
        severity = severity(default = "warning", categories = {"Security/": "error"}),
        support = [
            row(
                capability = "format",
                classes = ["ruby"],
                family = "ruby",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "RubyInstaller arm64 Ruby is not pinned or qualified",
                },
            ),
            row(
                capability = "lint",
                classes = ["ruby"],
                family = "ruby",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "RubyInstaller arm64 Ruby is not pinned or qualified",
                },
            ),
        ],
    ),
    tool(
        id = "sorbet",
        adapters = {"typecheck": ["ruby"]},
        support = [
            row(
                capability = "typecheck",
                classes = ["ruby"],
                context = "target_coupled",
                family = "ruby",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                unsupported = {
                    "windows_arm64": "Sorbet ships no Windows build",
                    "windows_x86_64": "Sorbet ships no Windows build",
                },
            ),
        ],
    ),
]
