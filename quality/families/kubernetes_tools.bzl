"""Quality tool rows for the kubernetes family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "helm",
        adapters = {"lint": ["helm"]},
        support = [
            row(
                capability = "lint",
                classes = ["helm"],
                family = "helm",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "helmfmt",
        adapters = {"format": ["helm"]},
        support = [
            row(
                capability = "format",
                classes = ["helm"],
                family = "helm",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "kubeconform",
        adapters = {"lint": ["kubernetes"]},
        concerns = {"lint": "schema"},
        config = config(suffixes = [".kubeconform.json"], files = ["kubernetes.kubeconform.json"], tier_active = True, tier_base = False, stage_native = True),
        severity = severity(default = "error"),
        support = [
            row(
                capability = "lint",
                classes = ["kubernetes"],
                family = "kubernetes",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
