"""sqlfluff entry point."""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool


def main() -> int:
    """Points the home directory at dx's scratch home, then runs sqlfluff."""
    home = os.environ.pop("DX_SQLFLUFF_HOME", "")
    if home:
        os.environ["HOME"] = home
        os.environ["USERPROFILE"] = home
    return import_tool("sqlfluff.cli.commands", "cli")()


if __name__ == "__main__":
    sys.exit(main())
