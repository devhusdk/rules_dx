"""Quality tool rows for the python family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "notebook_outputs",
        adapters = {"lint": ["jupyter"]},
        concerns = {"lint": "notebook_outputs"},
        support = [
            row(
                capability = "lint",
                classes = ["jupyter"],
                family = "jupyter",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "pydoclint",
        adapters = {"lint": ["python", "python_stub"]},
        concerns = {"lint": "docstrings"},
        config = config(suffixes = [".toml"], ext = "toml", comment = "#", tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["python", "python_stub"],
                family = "python",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "ruff",
        adapters = {
            "audit": ["python", "python_stub"],
            "format": ["python", "python_stub", "jupyter"],
            "lint": ["python", "python_stub", "jupyter"],
        },
        config = config(
            suffixes = [".toml"],
            files = ["ruff.toml", ".ruff.toml"],
            ext = "toml",
            extends_syntax = "# Repo settings go here; the extend line picks the profile.\nextend = \"{base}\"\n",
            comment = "#",
            pyproject_table = "ruff",
            references = ["toml:extend"],
            rule_keys = ["lint.select", "lint.ignore", "lint.per-file-ignores"],
            init_order = 1,
            family = "python",
            tier_active = True,
            suppressions = ["noqa"],
            suppression_extensions = [".py", ".pyi"],
        ),
        severity = severity(
            default = "warning",
            categories = {
                "E9": "error",
                "F82": "error",
                "S1": "error",
                "S2": "error",
                "S3": "error",
                "S4": "error",
                "S5": "error",
                "S6": "error",
                "S7": "error",
            },
        ),
        support = [
            row(
                capability = "format",
                classes = ["python", "python_stub"],
                family = "python",
                fix = "supported",
                state = "qualified",
            ),
            row(
                capability = "lint",
                classes = ["python", "python_stub"],
                family = "python",
                fix = "supported",
                state = "qualified",
            ),
            row(
                capability = "format",
                classes = ["jupyter"],
                family = "jupyter",
                fix = "supported",
                state = "qualified",
            ),
            row(
                capability = "lint",
                classes = ["jupyter"],
                family = "jupyter",
                fix = "supported",
                state = "qualified",
            ),
        ],
    ),
    tool(
        id = "ty",
        adapters = {"typecheck": ["jupyter", "python", "python_stub"]},
        config = config(suffixes = [".toml"], tier_active = True),
        severity = severity(default = "error"),
        support = [
            row(
                capability = "typecheck",
                classes = ["python", "python_stub"],
                context = "target_coupled",
                family = "python",
                fix = "check_only",
                state = "qualified",
            ),
            row(
                capability = "typecheck",
                classes = ["jupyter"],
                context = "target_coupled",
                family = "jupyter",
                fix = "check_only",
                state = "qualified",
            ),
        ],
    ),
]
