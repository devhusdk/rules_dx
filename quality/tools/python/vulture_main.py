"""Run the pinned vulture entry point; its main exits with the report status."""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

import_tool("vulture.core", "main")()
