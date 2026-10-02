"""Shared import bootstrap for the pinned Python quality tools.

The hermetic launcher starts the interpreter without the pinned venv on
Windows, so the wheel graph never reaches `sys.path` and every tool dies
with ModuleNotFoundError before it can parse anything. When that
happens, wire in the site-packages directories this tool's runfiles
already carry.

Only absolute manifest targets are read. Windows never materializes the
runfile link tree, so the relative entries cannot be resolved; the absolute
ones point at each wheel, either at a file inside its site-packages or at
the install root the wheel unpacks into.
"""

import glob
import os
import pathlib
import sys


def site_packages() -> list:
    """Returns every site-packages directory this tool's runfiles carry."""
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if not manifest:
        return []
    try:
        lines = pathlib.Path(manifest).read_text().splitlines()
    except OSError:
        return []
    sites = []

    def add(path):
        """Records one import root the runfiles carry."""
        if path.is_dir() and path not in sites:
            sites.append(path)

    for line in lines:
        _, _, target = line.partition(" ")
        target = target.strip()
        if not target:
            continue
        path = pathlib.Path(target)
        if not path.is_absolute():
            continue
        for parent in path.parents:
            if parent.name == "site-packages":
                add(parent)
                break
        if not path.is_dir():
            continue
        pattern = str(path) + os.sep + "**" + os.sep + "site-packages"
        for site in glob.glob(pattern, recursive=True):
            add(pathlib.Path(site))
    return sites


def import_tool(module: str, attribute: str):
    """Imports one attribute, wiring the pinned wheels in only when required."""
    try:
        return getattr(__import__(module, fromlist=[attribute]), attribute)
    except ModuleNotFoundError:
        for site in reversed(site_packages()):
            sys.path.insert(0, str(site))
        return getattr(__import__(module, fromlist=[attribute]), attribute)
