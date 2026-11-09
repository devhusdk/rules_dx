"""Thin entry library owning main.py."""

import helper


def message():
    return "entry " + helper.suffix("x")


if __name__ == "__main__":
    print(message())
