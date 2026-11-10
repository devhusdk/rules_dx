"""Quality tool rows for the fortran family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "ffmt",
        adapters = {"format": ["fortran"]},
        config = config(suffixes = [".toml"], files = ["ffmt.toml"], tier_active = True, stage_native = True),
        support = [
            row(
                capability = "format",
                classes = ["fortran"],
                family = "fortran",
                fix = "supported",
                platforms = ["linux_x86_64"],
            ),
        ],
    ),
    tool(
        id = "flang",
        adapters = {"typecheck": ["fortran"]},
    ),
    tool(
        id = "fortitude",
        adapters = {"lint": ["fortran"]},
        config = config(suffixes = [".toml"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["fortran"],
                family = "fortran",
                fix = "check_only",
                platforms = ["linux_x86_64"],
            ),
        ],
    ),
]
