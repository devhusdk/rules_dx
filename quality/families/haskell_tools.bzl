"""Quality tool rows for the haskell family."""

load("//quality:tool_defs.bzl", "config", "row", "tool")

TOOLS = [
    tool(
        id = "ghc",
        adapters = {"typecheck": ["haskell"]},
    ),
    tool(
        id = "hlint",
        adapters = {"lint": ["haskell"]},
        config = config(
            suffixes = [".yaml"],
            files = [".hlint.yaml"],
            ext = "yaml",
            comment = "#",
            init_order = 7,
            family = "haskell",
            tier_active = True,
        ),
        support = [
            row(
                capability = "lint",
                classes = ["haskell"],
                family = "haskell",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "windows_arm64", "windows_x86_64"],
                unsupported = {
                    "macos_arm64": "hlint v3.10 ships only x86_64-osx, and the managed GHC has no hermetic macOS cabal path yet",
                },
            ),
        ],
    ),
    tool(
        id = "ormolu",
        adapters = {"format": ["haskell"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["haskell"],
                family = "haskell",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
                state = "qualified",
                transport = "single",
            ),
        ],
    ),
    tool(
        id = "weeder",
        adapters = {"lint": ["haskell"]},
        concerns = {"lint": "unused_code"},
        support = [
            row(
                capability = "lint",
                classes = ["haskell"],
                context = "target_coupled",
                family = "haskell",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                transport = "single",
                unsupported = {
                    "windows_arm64": "weeder is built from Hackage and only the linux_x86_64 build is verified",
                    "windows_x86_64": "weeder is built from Hackage and only the linux_x86_64 build is verified",
                },
            ),
        ],
    ),
]
