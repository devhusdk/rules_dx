"""Quality tool rows for the nix family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "deadnix",
        adapters = {"lint": ["nix"]},
        config = config(suffixes = [".txt"], tier_active = True, tier_base = False),
        support = [
            row(
                capability = "lint",
                classes = ["nix"],
                family = "nix",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "nixfmt",
        adapters = {"format": ["nix"]},
        support = [
            row(
                capability = "format",
                classes = ["nix"],
                family = "nix",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64"],
                unsupported = {
                    "macos_arm64": "nixfmt builds from source with the managed GHC, which has no hermetic macOS cabal path yet",
                    "windows_arm64": "nixfmt depends on the unix package and does not build on Windows",
                    "windows_x86_64": "nixfmt depends on the unix package and does not build on Windows",
                },
            ),
        ],
    ),
]
