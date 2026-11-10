"""Quality tool rows for the jvm family."""

load("//core/quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "detekt",
        adapters = {"lint": ["kotlin"]},
        config = config(suffixes = [".yml", ".yaml"], tier_active = True),
        severity = severity(default = "warning"),
        support = [
            row(
                capability = "lint",
                classes = ["kotlin"],
                family = "kotlin",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "google_java_format",
        adapters = {"format": ["java"]},
        support = [
            row(
                capability = "format",
                classes = ["java"],
                family = "java",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "javac",
        severity = severity(default = "error"),
        adapters = {"typecheck": ["java"]},
        support = [
            row(
                capability = "typecheck",
                classes = ["java"],
                context = "target_coupled",
                family = "java",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "kotlinc",
        severity = severity(default = "error"),
        adapters = {"typecheck": ["kotlin"]},
        support = [
            row(
                capability = "typecheck",
                classes = ["kotlin"],
                context = "target_coupled",
                family = "kotlin",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "ktfmt",
        adapters = {"format": ["kotlin"]},
        config = config(kind = "none", tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["kotlin"],
                family = "kotlin",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "pmd",
        adapters = {"lint": ["java"]},
        config = config(suffixes = [".xml"], tier_active = True),
        severity = severity(
            default = "warning",
            categories = {
                "HardCodedCryptoKey": "error",
                "InsecureCryptoIv": "error",
            },
        ),
        support = [
            row(
                capability = "lint",
                classes = ["java"],
                family = "java",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "scalac",
        severity = severity(default = "error"),
        adapters = {"typecheck": ["scala"]},
        support = [
            row(
                capability = "typecheck",
                classes = ["scala"],
                context = "target_coupled",
                family = "scala",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "scalafix",
        adapters = {"lint": ["scala"]},
        config = config(suffixes = [".conf"], tier_active = True),
        support = [
            row(
                capability = "lint",
                classes = ["scala"],
                context = "target_coupled",
                family = "scala",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "scalafmt",
        adapters = {"format": ["scala"]},
        config = config(suffixes = [".conf"], files = [".scalafmt.conf"], tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["scala"],
                family = "scala",
                fix = "supported",
            ),
        ],
    ),
    tool(
        id = "spotbugs",
        adapters = {"lint": ["java"]},
        concerns = {"lint": "bytecode"},
        config = config(suffixes = [".xml"], tier_active = True),
        severity = severity(
            default = "warning",
            categories = {
                "ASE_": "error",
                "DMI_CONSTANT_DB_PASSWORD": "error",
                "DMI_EMPTY_DB_PASSWORD": "error",
                "HRS_": "error",
                "PT_": "error",
                "UNS_": "error",
                "USBC_": "error",
                "USO_": "error",
                "XSS_": "error",
            },
        ),
        support = [
            row(
                capability = "lint",
                classes = ["java"],
                context = "target_coupled",
                family = "java",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
]
