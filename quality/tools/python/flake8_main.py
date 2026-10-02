"""Flake8 entry point with exit-code propagation (WP3).

Upstream `py_console_script_binary` expands to `{fn}()` without `sys.exit`,
so flake8's integer return (1 findings, 2 usage error) is dropped and every
run exits 0. Quality actions require the exit-code contract (0 clean,
1 findings), so this wrapper preserves the wheel-only graph and shared
managed runtime while propagating the return code.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

if __name__ == "__main__":
    sys.exit(import_tool("flake8.main.cli", "main")())
