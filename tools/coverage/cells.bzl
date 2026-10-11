"""Coverage cell registry (single source)."""

COVERAGE_CELLS = {
    "seed-linux_x86_64": "tools/coverage/seed-inventory.txt",
    "linux_arm64": "tools/coverage/arm64-inventory.txt",
    "macos_arm64": "tools/coverage/macos-arm64-inventory.txt",
    "windows_x86_64": "tools/coverage/windows-x86_64-inventory.txt",
    "windows_arm64": "tools/coverage/windows-arm64-inventory.txt",
}

COVERAGE_SEED_CELL = "seed-linux_x86_64"

INVENTORY_FILES = [
    "arm64-inventory.txt",
    "cells.txt",
    "macos-arm64-inventory.txt",
    "seed-inventory.txt",
    "windows-arm64-inventory.txt",
    "windows-x86_64-inventory.txt",
]

def coverage_cells(name):
    """Declare the per-cell inventory filegroup from the registry."""
    native.filegroup(
        name = name,
        srcs = INVENTORY_FILES,
    )
