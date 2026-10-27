"""Shared visibility scopes for BUILD files."""

VIS_PUBLIC = ["//visibility:public"]

VIS_CLI = [
    "//cli:__pkg__",
    "//cli:__subpackages__",
]

VIS_CLI_DEPLOY_QUALITY = VIS_CLI + [
    "//deploy:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_DEPLOY_QUALITY_TOOLS = VIS_CLI_DEPLOY_QUALITY + ["//tools:__subpackages__"]

VIS_CLI_DEPLOY_GENERATION_QUALITY = VIS_CLI + [
    "//deploy:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_DEPLOY_ENV_GENERATION_QUALITY = VIS_CLI + [
    "//deploy:__subpackages__",
    "//env:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_TOOLS = VIS_CLI + [
    "//tools:__subpackages__",
]

VIS_CLI_ENV_GENERATION_QUALITY = VIS_CLI + [
    "//env:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_WIDE = VIS_CLI + [
    "//tools/docs:__subpackages__",
    "//env:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_CLI_PLATFORM = VIS_CLI_WIDE + [
    "//deploy:__subpackages__",
    "//tools:__subpackages__",
]

VIS_CLI_DOCS_GENERATION_QUALITY = VIS_CLI + [
    "//tools/docs:__subpackages__",
    "//generation:__subpackages__",
    "//quality:__subpackages__",
]

VIS_ENV_SHARD = VIS_CLI + [
    "//env:__pkg__",
    "//env:__subpackages__",
]

VIS_CLI_GENERATION = VIS_CLI + [
    "//generation:__pkg__",
]

VIS_QUALITY = [
    "//quality:__pkg__",
    "//quality:__subpackages__",
]

VIS_QUALITY_ENV = [
    "//env:__pkg__",
] + VIS_QUALITY

VIS_QUALITY_CLI = VIS_CLI + VIS_QUALITY

VIS_TOOLS = ["//tools/visibility:tools_repo"]

VIS_TOOLS_PUBLIC = ["//tools/visibility:tools_public"]

VIS_TOOLS_CLI_TEST = ["//tools/visibility:tools_cli_test"]

VIS_DX_ROOTS = ["//tools/visibility:dx_roots"]

VIS_DX_COMMANDS = ["//tools/visibility:dx_commands"]
