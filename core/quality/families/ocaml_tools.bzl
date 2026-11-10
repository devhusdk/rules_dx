"""Quality tool rows for the ocaml family."""

load("//core/quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "ocaml",
        adapters = {"typecheck": ["ocaml"]},
    ),
    tool(
        id = "topiary",
        adapters = {"format": ["ocaml"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["ocaml"],
                family = "ocaml",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "zanuda",
        adapters = {"lint": ["ocaml"]},
        config = config(suffixes = [".txt"], tier_active = True, tier_base = False),
        support = [
            row(
                capability = "lint",
                classes = ["ocaml"],
                context = "target_coupled",
                family = "ocaml",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                transport = "single",
                unsupported = {
                    "windows_arm64": "zanuda has no Windows build",
                    "windows_x86_64": "zanuda has no Windows build",
                },
            ),
        ],
    ),
]
