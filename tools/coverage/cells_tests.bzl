"""Coverage cell registry tests."""

load("//libs/starlark:defs.bzl", "starlark_test")

_CELL_ROWS = [
    "qualified seed-linux_x86_64 tools/coverage/seed-inventory.txt",
    "qualified linux_arm64 tools/coverage/arm64-inventory.txt",
    "qualified macos_arm64 tools/coverage/macos-arm64-inventory.txt",
    "qualified windows_x86_64 tools/coverage/windows-x86_64-inventory.txt",
    "qualified windows_arm64 tools/coverage/windows-arm64-inventory.txt",
]

_CELL_ENTRIES = [
    "\"seed-linux_x86_64\": \"tools/coverage/seed-inventory.txt\"",
    "\"linux_arm64\": \"tools/coverage/arm64-inventory.txt\"",
    "\"macos_arm64\": \"tools/coverage/macos-arm64-inventory.txt\"",
    "\"windows_x86_64\": \"tools/coverage/windows-x86_64-inventory.txt\"",
    "\"windows_arm64\": \"tools/coverage/windows-arm64-inventory.txt\"",
]

def coverage_cells_tests(name):
    """Pin the registry snapshot: cells.txt rows must equal COVERAGE_CELLS."""
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            ":cells.txt": "\n".join(_CELL_ROWS),
            ":cells.bzl": "\n".join(_CELL_ENTRIES + ["COVERAGE_SEED_CELL = \"seed-linux_x86_64\""]),
        },
    )
