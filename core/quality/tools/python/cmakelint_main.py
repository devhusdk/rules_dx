"""cmakelint entry point.

Upstream reads every file with the bare `open(filename)` builtin, which decodes with the
locale codec and breaks on Windows code pages. The module-level `open` shim below makes
`cmakelint.main` read sources and configs as strict UTF-8, and the standard streams are
written as UTF-8 so non-ASCII paths in findings survive.
"""

import builtins
import importlib
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool


def utf8_open(filename, mode="r", *args, **kwargs):
    """Opens text files as strict UTF-8; binary modes pass through."""
    if "b" not in mode:
        kwargs["encoding"] = "utf-8"
        kwargs["errors"] = "strict"
    return builtins.open(filename, mode, *args, **kwargs)


def main():
    """Runs cmakelint with UTF-8 file reads."""
    for stream in (sys.stdout, sys.stderr):
        reconfigure = getattr(stream, "reconfigure", None)
        if reconfigure is not None:
            reconfigure(encoding="utf-8")
    tool = import_tool("cmakelint.main", "main")
    importlib.import_module("cmakelint.main").open = utf8_open
    return tool()


if __name__ == "__main__":
    sys.exit(main())
