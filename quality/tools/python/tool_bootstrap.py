"""Shared import bootstrap for the pinned Python quality tools.

The hermetic launcher starts the interpreter without the pinned venv on
Windows, so the wheel graph never reaches `sys.path` and every tool dies
with ModuleNotFoundError before it can parse anything. When that
happens, wire in the site-packages directories this tool's runfiles
already carry.

Only absolute manifest targets are considered. On Windows the runfile
link tree is never materialized, and the entries that describe the pinned
wheels are relative to it, so they cannot be resolved; the absolute ones
point straight at the venv in its own execution configuration.
"""

import os
import pathlib
import sys


def site_packages() -> list:
    """Returns every absolute site-packages directory in this tool's runfiles."""
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    if not manifest:
        return []
    try:
        lines = pathlib.Path(manifest).read_text().splitlines()
    except OSError:
        return []
    sites = []
    for line in lines:
        _, _, target = line.partition(" ")
        target = target.strip()
        path = pathlib.Path(target) if target else None
        if path == None or not path.is_absolute():
            continue
        for parent in path.parents:
            if parent.name != "site-packages":
                continue
            if parent.is_dir() and parent not in sites:
                sites.append(parent)
            break
    return sites


def import_tool(module: str, attribute: str):
    """Imports one attribute, wiring the pinned wheels in only when required."""
    try:
        return getattr(__import__(module, fromlist=[attribute]), attribute)
    except ModuleNotFoundError:
        for site in reversed(site_packages()):
            sys.path.insert(0, str(site))
        return getattr(__import__(module, fromlist=[attribute]), attribute)