import os

if hasattr(os, "add_dll_directory"):
    os.add_dll_directory(os.path.dirname(os.path.abspath(__file__)))

from ._core import greet

__all__ = ["greet"]
