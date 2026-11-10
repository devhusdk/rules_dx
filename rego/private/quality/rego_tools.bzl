"""Quality tool rows for the rego family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "opa",
        adapters = {
            "format": ["rego"],
            "typecheck": ["rego"],
        },
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["rego"],
                family = "rego",
                fix = "supported",
                state = "qualified",
                transport = "single",
            ),
            row(
                capability = "typecheck",
                classes = ["rego"],
                context = "target_coupled",
                family = "rego",
                fix = "check_only",
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "regal",
        adapters = {"lint": ["rego"]},
        config = config(suffixes = [".yaml", ".yml"], files = [".regal/config.yaml"], tier_active = True, stage_native = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["rego"],
                family = "rego",
                fix = "check_only",
                state = "qualified",
            ),
        ],
    ),
]
