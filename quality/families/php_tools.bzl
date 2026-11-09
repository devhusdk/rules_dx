"""Quality tool rows for the php family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "phpcs",
        adapters = {"lint": ["php"]},
        config = config(suffixes = [".xml", ".xml.dist"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["php"],
                family = "php",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build",
                },
            ),
        ],
    ),
    tool(
        id = "phpstan",
        adapters = {
            "typecheck": ["php"],
        },
        config = config(suffixes = [".neon", ".neon.dist"], tier_active = True, tier_source = "phpstan level 6 and phpstan-strict-rules"),
        severity = severity(default = "error", categories = {"phpstan/public.": "info"}),
        support = [
            row(
                capability = "typecheck",
                classes = ["php"],
                context = "target_coupled",
                family = "php",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_x86_64"],
                unsupported = {
                    "windows_arm64": "static-php-cli and windows.php.net ship no Windows arm64 PHP 8.5 build",
                },
            ),
        ],
    ),
]
