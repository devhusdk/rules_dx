"""Runs `griffe check <package> --against <ref> --search <dir>` on the pinned griffe library.

Each breaking change prints as `path.py:line: object: kind`. The exit code is 1
when there is any breaking change and 0 otherwise.
"""

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from tool_bootstrap import import_tool


def parse(argv):
    """Reads the arguments the dry run passes to griffe check."""
    parser = argparse.ArgumentParser(prog="griffe_check")
    parser.add_argument("command", choices=["check"])
    parser.add_argument("package")
    parser.add_argument("--against", required=True)
    parser.add_argument("--search", action="append", default=[])
    return parser.parse_args(argv)


def breaking_lines(package, against, search):
    """Returns one line per breaking change between the ref and the working tree."""
    load = import_tool("griffe", "load")
    load_git = import_tool("griffe", "load_git")
    find = import_tool("griffe", "find_breaking_changes")
    old = load_git(package, ref=against, search_paths=search)
    new = load(package, search_paths=search)
    lines = []
    for change in find(old, new):
        path = change.obj.filepath
        lines.append(f"{path}:{change.obj.lineno or 0}: {change.obj.path}: {change.kind.value}")
    return lines


def main(argv):
    """Prints the breaking changes and returns the exit code."""
    args = parse(argv)
    lines = breaking_lines(args.package, args.against, args.search)
    for line in lines:
        print(line)
    return 1 if lines else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
