"""Shared import bootstrap for the pinned Python quality tools."""

import glob
import os
import pathlib
import sys


def site_packages() -> list:
    """Returns every site-packages directory this tool's runfiles carry."""
    sites = []
    manifest = os.environ.get("RUNFILES_MANIFEST_FILE")
    lines = []
    if manifest:
        try:
            lines = pathlib.Path(manifest).read_text().splitlines()
        except OSError:
            lines = []
    root = os.environ.get("RUNFILES_DIR")
    if root and os.path.isdir(root):
        for site in glob.glob(os.path.join(root, "**", "site-packages"), recursive=True):
            sites.append(pathlib.Path(site))
    if not manifest and not sites:
        return []

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
