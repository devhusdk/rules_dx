"""Quality tool rows for the beam family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "credo",
        adapters = {"lint": ["elixir"]},
        config = config(suffixes = [".exs"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["elixir"],
                family = "elixir",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "dialyzer",
        adapters = {"typecheck": ["erlang", "elixir"]},
        severity = severity(default = "error"),
        support = [
            row(
                capability = "typecheck",
                classes = ["erlang"],
                context = "target_coupled",
                family = "erlang",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            row(
                capability = "typecheck",
                classes = ["elixir"],
                context = "target_coupled",
                family = "elixir",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                unsupported = {
                    "windows_arm64": "dialyzer on Windows is pending #6321",
                    "windows_x86_64": "dialyzer on Windows is pending #6008",
                },
            ),
        ],
    ),
    tool(
        id = "elvis",
        adapters = {"lint": ["erlang"]},
        config = config(suffixes = ["elvis.config", ".config"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["erlang"],
                family = "erlang",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "elixirc",
        adapters = {"typecheck": ["elixir"]},
        concerns = {"typecheck": "unused_code"},
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "typecheck",
                classes = ["elixir"],
                context = "target_coupled",
                family = "elixir",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64"],
                unsupported = {
                    "windows_arm64": "The elixirc replay needs a POSIX shell",
                    "windows_x86_64": "The elixirc replay needs a POSIX shell",
                },
            ),
        ],
    ),
    tool(
        id = "erlc",
        adapters = {"typecheck": ["erlang"]},
        concerns = {"typecheck": "unused_code"},
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "typecheck",
                classes = ["erlang"],
                context = "target_coupled",
                family = "erlang",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "erlfmt",
        adapters = {"format": ["erlang"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["erlang"],
                family = "erlang",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "mix_format",
        adapters = {"format": ["elixir"]},
        config = config(
            suffixes = [".exs"],
            files = [".formatter.exs"],
            ext = "exs",
            comment = "#",
            tier_active = True,
        ),
        support = [
            row(
                capability = "format",
                classes = ["elixir"],
                family = "elixir",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
