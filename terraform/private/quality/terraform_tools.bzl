"""Quality tool rows for the terraform family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "terraform",
        adapters = {
            "format": ["terraform"],
            "typecheck": ["terraform"],
        },
        support = [
            row(
                capability = "format",
                classes = ["terraform"],
                family = "terraform",
                fix = "supported",
                state = "qualified",
                transport = "single",
            ),
            row(
                capability = "typecheck",
                classes = ["terraform"],
                context = "target_coupled",
                family = "terraform",
                fix = "check_only",
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "tflint",
        adapters = {"lint": ["terraform"]},
        config = config(suffixes = [".hcl"], tier_active = True),
        severity = severity(default = "warning", categories = {"tflint-diagnostic": "error"}),
        support = [
            row(
                capability = "lint",
                classes = ["terraform"],
                family = "terraform",
                fix = "check_only",
            ),
        ],
    ),
]
