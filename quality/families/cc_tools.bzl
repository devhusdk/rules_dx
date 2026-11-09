"""Quality tool rows for the cc family."""

load("//quality:tool_defs.bzl", "config", "row", "severity", "tool")

TOOLS = [
    tool(
        id = "clang_format",
        adapters = {"format": ["c", "cpp", "cuda", "objc", "objcpp"]},
        config = config(suffixes = [".clang-format"], files = [".clang-format"], tier_active = True),
        support = [
            row(
                capability = "format",
                classes = ["c", "cpp"],
                family = "cc",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            row(
                capability = "format",
                classes = ["cuda"],
                family = "cuda",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            row(
                capability = "format",
                classes = ["objc", "objcpp"],
                family = "objc",
                fix = "supported",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "clang_syntax",
        adapters = {"typecheck": ["c", "cpp", "objc", "objcpp"]},
        support = [
            row(
                capability = "typecheck",
                classes = ["c", "cpp"],
                context = "target_coupled",
                family = "cc",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
            row(
                capability = "typecheck",
                classes = ["objc", "objcpp"],
                context = "target_coupled",
                family = "objc",
                fix = "check_only",
                platforms = ["macos_arm64"],
                unsupported = {
                    "linux_arm64": "objc_library builds only with an Apple toolchain",
                    "linux_x86_64": "objc_library builds only with an Apple toolchain",
                    "windows_arm64": "objc_library builds only with an Apple toolchain",
                    "windows_x86_64": "objc_library builds only with an Apple toolchain",
                },
            ),
        ],
    ),
    tool(
        id = "clang_tidy",
        adapters = {"lint": ["c", "cpp", "cuda", "objc", "objcpp"]},
        config = config(suffixes = [".clang-tidy"], tier_active = True),
        severity = severity(categories = {
            "bugprone-": "warning",
            "cert-": "warning",
            "clang-analyzer-": "warning",
            "clang-diagnostic-error": "error",
            "cppcoreguidelines-": "warning",
            "misc-": "warning",
            "modernize-": "warning",
            "performance-": "warning",
            "readability-": "warning",
        }),
        support = [
            row(
                capability = "lint",
                classes = ["c", "cpp"],
                context = "target_coupled",
                family = "cc",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "macos_arm64", "windows_arm64", "windows_x86_64"],
            ),
        ],
    ),
    tool(
        id = "nvcc_syntax",
        adapters = {"typecheck": ["cuda"]},
        support = [
            row(
                capability = "typecheck",
                classes = ["cuda"],
                context = "target_coupled",
                family = "cuda",
                fix = "check_only",
                platforms = ["linux_arm64", "linux_x86_64", "windows_x86_64"],
            ),
        ],
    ),
]
