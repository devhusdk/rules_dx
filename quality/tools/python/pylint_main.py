"""Pylint entry point with exit-code propagation (WP3).

Upstream `py_console_script_binary` expands to `{fn}()` without `sys.exit`,
so pylint's bit-encoded return (1 fatal, 2 error, 4 warning, 8 refactor,
16 convention, 32 usage error) is dropped and every run exits 0. Quality
actions require the exit-code contract, so this wrapper preserves the
wheel-only graph and shared managed runtime while propagating the return
code. If `run_pylint` raises SystemExit itself, it propagates unchanged.
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

if __name__ == "__main__":
    sys.exit(import_tool("pylint", "run_pylint")())
