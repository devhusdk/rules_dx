"""rst2html entry point for Vale's reStructuredText conversion."""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool


def main() -> None:
    """Runs docutils' HTML publisher over the command line arguments."""
    publish_cmdline = import_tool("docutils.core", "publish_cmdline")
    publish_cmdline(writer="html", description="Converts reStructuredText to HTML.")


if __name__ == "__main__":
    main()
