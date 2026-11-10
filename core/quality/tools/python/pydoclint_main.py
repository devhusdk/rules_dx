"""Run the pinned pydoclint entry point with exit-code propagation."""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

sys.exit(import_tool("pydoclint.main", "main")())
