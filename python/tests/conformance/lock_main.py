"""Prints a greeting after importing a locked package."""

import pytest

if __name__ == "__main__":
    assert pytest.__name__ == "pytest"
    print("hello lock")
