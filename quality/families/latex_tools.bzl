"""Quality tool rows for the latex family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "chktex",
        adapters = {"lint": ["latex"]},
        config = config(kind = "none", tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["latex"],
                family = "latex",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "latexindent",
        adapters = {"format": ["latex"]},
        config = config(suffixes = [".yaml", ".yml"], tier_active = True, tier_base = False),
        support = [
            row(
                capability = "format",
                classes = ["latex"],
                family = "latex",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
]
