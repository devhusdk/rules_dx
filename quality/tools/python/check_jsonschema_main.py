"""check-jsonschema entry point."""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool

if __name__ == "__main__":
    sys.exit(import_tool("check_jsonschema", "main")())
